use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

use super::ConfirmedSubscriber;
use super::Newsletter;
use super::PublishError;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;

/// `POST /admin/newsletters`
#[tracing::instrument(
    name = "Publishing newsletter",
    skip(form, pool, email_client),
    // `Empty` indicates that the value of a field is not currently present but will be recorded
    // later (with `Span.record`).
    fields(
        username=tracing::field::Empty,
        user_id=tracing::field::Empty,
    )
)]
pub async fn publish(
    // body: web::Json<Newsletter>,
    form: web::Form<Newsletter>,
    // like in `subscribe`
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    // request: HttpRequest,
    // _user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    // let creds =
    // basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

    // tracing::Span::current().record(
    //     "username",
    //     // &creds.username,
    //     tracing::field::display(&creds.username),
    // );
    //
    // let id = validate_credentials(creds, &pool)
    //     .await
    //     // AuthError can be mapped to PublishError 1:1
    //     .map_err(|e| match e {
    //         AuthError::UnexpectedError(_) =>
    // PublishError::UnexpectedError(e.into()),
    //         AuthError::InvalidCredentials(_) =>
    // PublishError::AuthError(e.into()),     })?;
    //
    // tracing::Span::current().record("user_id", tracing::field::display(id));

    let subs = get_confirmed_subscribers(&pool).await?;
    for sub in subs {
        match sub {
            Ok(sub) => email_client
                .send_email(
                    &sub.email,
                    &form.title,
                    &form.content,
                    &form.content,
                    // &body.content.text,
                )
                .await
                // `with_context` is lazy, and is preferred when the context is not static
                // note: a single send_email failure terminates the entire loop prematurely!
                .with_context(|| format!("could not send newsletter to {}", sub.email))?,
            Err(e) => tracing::warn!(
                e.cause_chain=?e,
                "skipping invalid email"
            ),
        }
    }
    FlashMessage::info("New issue published successfully.").send();
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Getting list of confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // recall: in subscriptions, we received `FormData` and, upon parsing, coerced
    // it into our own struct `NewSubscriber`

    // let rows = sqlx::query_as!(
    // `query_as` coerces the retrieved rows into a desired type; in our case, we
    // only need the `email` field, and skip the others to reduce data. in any
    // case, type conversions are better done separately anyway (see below)
    let subs = sqlx::query!(
        r#"
        SELECT email FROM subscriptions
        WHERE status = 'confirmed'
    "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    // although user emails should have been parsed when they were added to the db, we
    // cannot assume that they are (still) valid when retrieved. we -could- do a few things:
    //
    // 1. ignore errors (and potentially blow up)
    // .map(|r| ConfirmedSubscriber {
    //     email: SubscriberEmail::parse(r.email).unwrap(),
    // })
    //
    // 2. skip invalid emails
    // .flat_map(|r| SubscriberEmail::parse(r.email)) // clippy told me to do this
    // .map(|r| ConfirmedSubscriber { email: r })
    //
    // 3. propagate errors up and let caller decide
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(error) => Err(anyhow::anyhow!(error)),
    })
    .collect();

    Ok(subs)
}
