use std::time::Duration;

use sqlx::Executor;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;
use uuid::Uuid;

use crate::configuration::Settings;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::startup::get_connection_pool;

/// Not to be confused with `NewsletterForm`!
pub struct Newsletter {
    title: String,
    content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(
    pool: &PgPool,
    issue_id: Uuid,
) -> Result<Newsletter, anyhow::Error> {
    let issue = sqlx::query_as!(
        Newsletter,
        r#"
        SELECT title, content
        FROM newsletter_issues
        WHERE newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}

/// To be run as a separate worker, outside the main API
pub async fn init_delivery_worker(cfg: Settings) -> Result<(), anyhow::Error> {
    // let sender_email = cfg.email_client.sender().unwrap();
    // let timeout = cfg.email_client.timeout();
    // let email_client = EmailClient::new(
    //     cfg.email_client.base_url,
    //     sender_email,
    //     cfg.email_client.authorization_token,
    //     timeout,
    // );

    let email_client = cfg.email_client.client();
    let pool = get_connection_pool(&cfg.database);
    send_email_loop(&pool, email_client).await
}

async fn send_email_loop(
    pool: &PgPool,
    email_client: EmailClient,
) -> Result<(), anyhow::Error> {
    loop {
        match try_send_email(pool, &email_client).await {
            Err(_) => tokio::time::sleep(Duration::from_secs(1)).await,
            Ok(DeliveryOutcome::NoTasksLeft) => tokio::time::sleep(Duration::from_secs(10)).await,
            Ok(DeliveryOutcome::TasksLeft) => {} // start next delivery immediately
        }
    }
}

pub enum DeliveryOutcome {
    NoTasksLeft,
    TasksLeft,
}

#[tracing::instrument(
    skip_all,
    fields(
        issue_id=tracing::field::Empty,
        email=tracing::field::Empty,
    ),
    err
)]
pub async fn try_send_email(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<DeliveryOutcome, anyhow::Error> {
    let task = start_delivery(pool).await?;

    if task.is_none() {
        return Ok(DeliveryOutcome::NoTasksLeft);
    }

    let (mut transaction, issue_id, email) = task.unwrap();

    tracing::Span::current()
        .record("issue_id", tracing::field::display(issue_id))
        .record("email", tracing::field::display(&email));

    // send

    let issue = get_issue(pool, issue_id).await?;

    match SubscriberEmail::parse(email.clone()) {
        Ok(email) => {
            while let Err(e) = email_client
                .send_email(&email, &issue.title, &issue.content, &issue.content)
                .await
            // // `with_context` is lazy, and is preferred when the context is
            // // not static
            // .with_context(|| format!("could not send newsletter to {}", email))
            // .map_err(error_500)?, // "cannot be shared across threads"
            {
                tracing::error!(
                    e.cause_chain=?e,
                    // e.message=%e,
                    "failed to deliver to {email}" //, retrying in {seconds} seconds..."
                );

                // everything below is beyond the scope of the book (and potentially
                // unnecessary); i wanted to put it in a function, but `transaction` is very
                // hard to pass around (we still need it for `finish_delivery`)

                let row = sqlx::query!(
                    r#"
                        SELECT n_retries, execute_after
                        FROM issue_delivery_queue
                        WHERE
                            newsletter_issue_id = $1 AND
                            subscriber_email = $2
                        "#,
                    issue_id,
                    email.as_ref()
                )
                .fetch_one(&mut *transaction)
                .await?;

                // i forgot to declare NOT NULL
                let retries = row.n_retries.unwrap() + 1;
                let seconds = retries * row.execute_after.unwrap();

                if seconds > 5000 {
                    return Err(anyhow::anyhow!("aborting after {retries} retries!"));
                }

                tokio::time::sleep(Duration::from_secs(seconds as u64)).await;

                sqlx::query!(
                    r#"
                        UPDATE issue_delivery_queue
                        SET
                            n_retries = $1,
                            execute_after = $2
                        WHERE
                            newsletter_issue_id = $3 AND
                            subscriber_email = $4
                        "#,
                    retries,
                    seconds,
                    issue_id,
                    email.as_ref()
                )
                .execute(&mut *transaction)
                .await?;
            }
        }

        Err(e) => tracing::warn!(
            e.cause_chain=?e,
            // e.message=%e,
            "skipping invalid email"
        ),
    }

    finish_delivery(transaction, issue_id, &email).await?;

    Ok(DeliveryOutcome::TasksLeft)
}

type PgTransaction = Transaction<'static, Postgres>;

/// Dequeue an entry in `issue_delivery_queue`
async fn start_delivery(
    pool: &PgPool
) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut transaction = pool.begin().await?;
    let query = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue

        FOR UPDATE -- lock currently selected row
        SKIP LOCKED -- don't select currently locked rows

        LIMIT 1
        "#
    );

    // let result = transaction
    //     .fetch_optional(query) // Executor
    //     .await?
    //     // PgRows don't have fields!
    //     .map(|r| (transaction, r.newsletter_issue_id, r.subscriber_email));

    // https://github.com/LukeMathWalker/zero-to-production/blob/a48a2a24720f820432a33b070c807b2f448b625f/src/issue_delivery_worker.rs#L89
    let result = query
        .fetch_optional(&mut *transaction)
        .await?
        .map(|r| (transaction, r.newsletter_issue_id, r.subscriber_email));

    Ok(result)
}

/// This is the last action in the transaction
async fn finish_delivery(
    // https://users.rust-lang.org/t/solved-placement-of-mut-in-function-parameters/19891
    mut transaction: PgTransaction, // mutable transaction
    // transaction: &mut PgTransaction, // mutable reference
    issue_id: Uuid,
    subscriber_email: &str,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE
            newsletter_issue_id = $1 AND
            subscriber_email = $2
        "#,
        issue_id,
        subscriber_email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}
