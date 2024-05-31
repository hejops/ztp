use std::fmt::Debug;

use actix_web::http::header;
use actix_web::http::header::HeaderValue;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::save_response;
use crate::idempotency::try_save_response;
use crate::idempotency::IdempotencyKey;
use crate::idempotency::NextAction;
use crate::routes::error_chain_fmt;
use crate::utils::error_400;
use crate::utils::error_500;
use crate::utils::redirect;

#[derive(Deserialize)]
pub struct Newsletter {
    title: String,
    // content: NewsletterContent,
    content: String,
    idempotency_key: String,
}

// #[derive(Deserialize)]
// struct NewsletterContent {
//     html: String,
//     text: String,
// }

struct ConfirmedSubscriber {
    // email: String,
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    // #[error("{0}")]
    // ValidationError(String),
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for PublishError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        error_chain_fmt(self, f)?;
        Ok(())
    }
}

impl ResponseError for PublishError {
    // fn status_code(&self) -> StatusCode {
    //     match self {
    //         Self::AuthError(_) => StatusCode::UNAUTHORIZED,
    //         _ => StatusCode::INTERNAL_SERVER_ERROR, // 500
    //     }
    // }

    // supersedes `status_code`
    fn error_response(&self) -> HttpResponse<actix_web::body::BoxBody> {
        match self {
            Self::AuthError(_) => {
                let mut resp = HttpResponse::new(StatusCode::UNAUTHORIZED); // 401
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                resp.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                resp
            }
            _ => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR), // 500
        }
    }
}

// note: POST is, per the spec, -not- idempotent. however, striving for
// idempotence makes the API call easier to use
//
// https://www.rfc-editor.org/rfc/rfc2616#section-9.1.2
//
// callers of any request (regardless of method) must not be expected to know
// anything about the underlying domain (e.g. previous calls, logs), and callees
// must respond to retries in a semantically equivalent way (e.g. status code)
//
// to distinguish initial tries and retries, we can use idempotency keys:
//
// - two identical requests, different idempotency keys = two distinct
//   operations
// - two identical requests, same idempotency key = a single operation, the
//   second request is a duplicate
// - two different8 requests, same idempotency key = the first request is
//   processed, the second one is rejected

// additionally, if one request has been initiated, similar requests should be
// rejected (409)/deferred until the first request finishes processing. however,
// note that browsers do not retry 409s, so deferral is preferred.

// stateful idempotence: store key:response as a hashmap and reuse response when
// the same key is received. however, domain changes (e.g. new subscriber) may
// be ignored, leading to undesirable skips (e.g. new subscriber won't receive
// existing newsletter)
//
// stateless idempotence: deterministically generate a key from the contents of
// the received request (like git sha), then forward it to the API provider
// (e.g. Postmark) and leave idempotence to them.
//
// for the purposes of doing it ourselves (and because Postmark doesn't do
// handle idempotence itfp), we use the stateful approach

// like sessions, idempotency keys are transient (don't need to persist) and
// isolated. for reasons yet unknown, redis is not suitable, so we use postgres
// with transactions

/// `POST /admin/newsletters`
///
/// Authentication is required, but this is handled by the
/// `reject_anonymous_users` middleware.
// if `form` cannot be Deserialized, returns 400 automatically
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
pub async fn publish_newsletter(
    // body: web::Json<Newsletter>,
    form: web::Form<Newsletter>,
    // like in `subscribe`
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    // request: HttpRequest,
    user_id: web::ReqData<UserId>,
    // ) -> Result<HttpResponse, PublishError> {
) -> Result<HttpResponse, actix_web::Error> {
    // let creds =
    // basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

    // auth now handled by reject_anonymous_users

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

    let user_id = user_id.into_inner();

    // note: the idempotency_key is embedded in `form`, so the book destructures the
    // `Newsletter` struct to pull the key out, but i'm lazy so i just clone the
    // field
    let key: IdempotencyKey = form.idempotency_key.clone().try_into().map_err(error_400)?;

    // // if let Ok(Some(saved)) = get_saved_response(*user_id, &key, &pool).await {
    // if let Some(saved) = get_saved_response(*user_id, &key, &pool)
    //     .await
    //     // more explicit error handling
    //     .map_err(error_500)?
    // {
    //     FlashMessage::info("Issue has already been published.").send();
    //     return Ok(saved);
    // };

    // "`READ COMMITTED` is the default isolation level in Postgres." this means:
    // - "fetching" callers will never see partially written (uncommitted) changes
    // - "modifying" callers are made to wait until any ongoing transaction is
    //   committed (or aborted)
    //
    // https://www.postgresql.org/docs/current/transaction-iso.html

    // 1. complete response saved -> return it
    // 2. incomplete response saved (a concurrent request was made) -> abort (DO
    //    NOTHING)
    // 3. no response saved -> proceed

    let transaction = match try_save_response(*user_id, &key, &pool)
        .await
        .map_err(error_500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved) => {
            FlashMessage::info("Issue has already been published.").send();
            return Ok(saved);
        }
    };

    let subs = get_confirmed_subscribers(&pool).await.map_err(error_500)?;
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
                // note: a single send_email failure terminates the -entire- loop prematurely!
                .with_context(|| format!("could not send newsletter to {}", sub.email))
                .map_err(error_500)?,
            Err(e) => tracing::warn!(
                e.cause_chain=?e,
                "skipping invalid email"
            ),
        }
    }
    FlashMessage::info("New issue published successfully.").send();

    // Ok(HttpResponse::Ok().finish())
    // Ok(redirect("/admin/newsletters"))

    // redirect first, then save the response (redirect) for idempotence purposes.
    // if retrieved, this fn will return early with a different message
    let resp = redirect("/admin/newsletters");
    let resp = save_response(*user_id, &key, resp, transaction)
        .await
        .map_err(error_500)?;
    Ok(resp)
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
