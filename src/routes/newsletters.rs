use std::fmt::Debug;

use actix_web::http::header;
use actix_web::http::header::ContentType;
use actix_web::http::header::HeaderValue;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use actix_web_flash_messages::FlashMessage;
use actix_web_flash_messages::IncomingFlashMessages;
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;

use super::error_chain_fmt;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;

#[derive(Deserialize)]
pub struct Newsletter {
    title: String,
    // content: NewsletterContent,
    content: String,
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

/// Parse headers of a HTTP request. This does not actually validate any user
/// credentials; for that, see `validate_credentials`.
// fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
//     // authentication methods fall in three categories: passwords / objects /
//     // biometrics. because there are drawbacks associated with each, multi-factor
//     // authentication is recommended
//
//     // spec: RFCs 2617, 7617
//     // - correct header ("Authorization")
//     // - correct realm ("publish")
//     // - correct username/password
//
//     let encoded = headers
//         .get("Authorization")
//         .context("No Authorization header")?
//         .to_str()
//         .context("Invalid str")?
//         .strip_prefix("Basic ")
//         .context("Authorization scheme was not 'Basic'")?;
//
//     let decoded = base64::engine::general_purpose::STANDARD
//         .decode(encoded)
//         .context("Failed to decode base64")?;
//     let decoded = String::from_utf8(decoded).context("Invalid str")?;
//
//     let mut creds = decoded.splitn(2, ':');
//
//     let username = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No username"))?
//         .to_string();
//
//     let password = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No password"))?
//         .to_string();
//     let password = Secret::new(password);
//
//     Ok(Credentials { username, password })
// }

/// `GET /admin/newsletters`
pub async fn newsletter_form(
    flash_messages: IncomingFlashMessages
) -> Result<HttpResponse, actix_web::Error> {
    let mut error_msg = String::new();
    for msg in flash_messages.iter() {
        //.filter(|m| m.level() == Level::Error) {
        // the book calls `writeln!(String)`, which is no longer allowed?
        // writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
        error_msg.push_str(&format!("<p><i>{}</i></p>\n", msg.content()))
    }

    // the book uses 2 input boxes for content (text/html), but i don't feel like
    // doing this

    let body = format!(
        r#"
<!doctype html>
<html lang="en">
  <head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8" />
    <title>Submit new issue</title>
  </head>
  {error_msg}
  <body>
    <form action="/admin/newsletters" method="post">
      <label>
        Title
        <input type="text" placeholder="Enter Title" name="title" />
      </label>
      <label>
        Content
        <input type="text" placeholder="Enter Content" name="content" />
      </label>
      <button type="submit">Submit</button>
    </form>
  </body>
</html>
    "#
    );

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}

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
