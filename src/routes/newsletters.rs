use std::fmt::Debug;

use actix_web::http::header;
use actix_web::http::header::HeaderMap;
use actix_web::http::header::HeaderValue;
use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use anyhow::Context;
use argon2::password_hash::Salt;
use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordHasher;
use argon2::PasswordVerifier;
use base64::engine::general_purpose;
use base64::Engine;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::error_chain_fmt;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;

#[derive(Deserialize)]
pub struct Newsletter {
    title: String,
    content: NewsletterContent,
}

#[derive(Deserialize)]
struct NewsletterContent {
    html: String,
    text: String,
}

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
                let mut resp = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                resp.headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                resp
            }
            _ => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR), // 500
        }
    }
}

struct Credentials {
    username: String,
    password: Secret<String>,
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // authentication methods fall in three categories: passwords / objects /
    // biometrics. because there are drawbacks associated with each, multi-factor
    // authentication is recommended

    // spec: RFCs 2617, 7617
    // - correct header ("Authorization")
    // - correct realm ("publish")
    // - correct username/password

    let encoded = headers
        .get("Authorization")
        .context("No Authorization header")?
        .to_str()
        .context("Invalid str")?
        .strip_prefix("Basic ")
        .context("Authorization scheme was not 'Basic'")?;

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("Failed to decode base64")?;
    let decoded = String::from_utf8(decoded).context("Invalid str")?;

    let mut creds = decoded.splitn(2, ':');

    let username = creds
        .next()
        .ok_or_else(|| anyhow::anyhow!("No username"))?
        .to_string();

    let password = creds
        .next()
        .ok_or_else(|| anyhow::anyhow!("No password"))?
        .to_string();
    let password = Secret::new(password);

    Ok(Credentials { username, password })
}

// on salting: "For each user, we generate a unique random string (salt), which
// is prepended to the user password before generating the hash." The salt does
// not need to be hashed, just random.
//
// this means we need to validate in 2 steps: first query `users` table to get
// the user's salt, then use the salt to calculate the hashed password
//
// however, notice now that, in comparison to our initial sha3 implementation,
// hashing now involves several parameters, which are defined in this function.
// if we were to change any of these parameters, and lose information on the
// params used to generate the existing hashes, authentication would break
// completely. thus, all such variable params must be stored in the db
// in PHC format, which captures all necessary information in a
// single string:
//
// # ${algo}${algo version}${params (,-separated)}${hash}${salt}
// (with newlines for clarity)
// $argon2id$v=19$m=65536,t=2,p=1
// $gZiV/M1gPc22ElAH/Jh1Hw
// $CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno
async fn validate_credentials(
    creds: Credentials,
    pool: &PgPool,
) -> Result<Uuid, PublishError> {
    let stored = sqlx::query!(
        "
        SELECT user_id, password_hash -- , salt
        FROM users
        WHERE username = $1
        -- AND password_hash = $2
    ",
        creds.username,
        // format!("{password_hash:x}"), // GenericArray -> hexadecimal
    )
    .fetch_optional(pool)
    .await
    .context("No user with the supplied username was found in users table")
    .map_err(PublishError::UnexpectedError)?
    .ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

    // use sha3::Digest;
    // use sha3::Sha3_256;
    // let hashed_pw = Sha3_256::digest(creds.password.expose_secret().as_bytes());

    // let hashed_pw = Argon2::new(
    //     // OWASP recommendation
    //     argon2::Algorithm::Argon2id,
    //     argon2::Version::V0x13,
    //     argon2::Params::new(15000, 2, 1, None).unwrap(),
    // )
    // // requires `PasswordHasher` trait.
    // // note: argon2 0.5.3 no longer allows the `salt` arg to be a `&str`, hence
    // // the lengthy type conversion (String -> bytes -> b64 -> Result<Salt>)
    // .hash_password(
    //     creds.password.expose_secret().as_bytes(),
    //     // &stored.salt
    //     Salt::from_b64(&general_purpose::STANDARD.encode(stored.salt.
    // as_bytes())).unwrap(), )
    // .unwrap();

    let hashed_pw = PasswordHash::new(&stored.password_hash)
        .context("Failed to read PHC string")
        .map_err(PublishError::UnexpectedError)?;

    Argon2::default()
        .verify_password(creds.password.expose_secret().as_bytes(), &hashed_pw)
        .context("Invalid password")
        .map_err(PublishError::AuthError)?;

    Ok(stored.user_id)
}

#[tracing::instrument(
    name = "Publishing newsletter",
    skip(body, pool, email_client, request),
    // `Empty` indicates that the value of a field is not currently present but will be recorded
    // later (with `Span.record`).
    fields(
        username=tracing::field::Empty,
        user_id=tracing::field::Empty,
    )
)]
pub async fn publish(
    body: web::Json<Newsletter>,
    // like in `subscribe`
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let creds = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

    tracing::Span::current().record(
        "username",
        // &creds.username,
        tracing::field::display(&creds.username),
    );

    let id = validate_credentials(creds, &pool).await?;

    tracing::Span::current().record("user_id", tracing::field::display(id));

    let subs = get_confirmed_subscribers(&pool).await?;
    for sub in subs {
        match sub {
            Ok(sub) => email_client
                .send_email(
                    &sub.email,
                    &body.title,
                    &body.content.html,
                    &body.content.text,
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
