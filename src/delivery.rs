use sqlx::Executor;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;
use uuid::Uuid;

use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;

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
    email_client: EmailClient,
) -> Result<(), anyhow::Error> {
    if let Some(result) = dequeue(pool).await? {
        let (transaction, issue_id, email) = result;

        tracing::Span::current()
            .record("issue_id", tracing::field::display(issue_id))
            .record("email", tracing::field::display(&email));

        // send

        let issue = get_issue(pool, issue_id).await?;

        match SubscriberEmail::parse(email.clone()) {
            Ok(email) => {
                if let Err(e) = email_client
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
                        "failed to deliver to {email}"
                    )
                }
            }

            Err(e) => tracing::warn!(
                e.cause_chain=?e,
                // e.message=%e,
                "skipping invalid email"
            ),
        }

        delete_task(transaction, issue_id, &email).await?;
    }
    Ok(())
}

type PgTransaction = Transaction<'static, Postgres>;

async fn dequeue(pool: &PgPool) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
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

async fn delete_task(
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
