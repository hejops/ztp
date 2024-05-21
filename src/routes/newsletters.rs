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
use argon2::Argon2;
use argon2::PasswordHash;
use argon2::PasswordVerifier;
use base64::Engine;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::task::JoinHandle;
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

// any API can expect to encounter 3 types of clients, each with different modes
// of authentication:
//
// 1. another API (machine) -- request signing, mutual TLS, OAuth2, JWT
// 2. another API (human) -- OAuth2 (scoped)
// 3. browser (human) -- session-based authentication (login form), identity
//    federation
//
// #3 will be our main target

struct Credentials {
    username: String,
    password: Secret<String>,
}

/// Parse headers of a HTTP request. This does not actually validate any user
/// credentials; for that, see `validate_credentials`.
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

async fn get_stored_credentials(
    username: String,
    pool: &PgPool,
    // returning `Record` is not allowed, unfortunately...
) -> Result<(Uuid, Secret<String>), PublishError> {
    let row = sqlx::query!(
        "
        SELECT user_id, password_hash -- , salt
        FROM users
        WHERE username = $1
        -- AND password_hash = $2
    ",
        username,
        // format!("{password_hash:x}"), // GenericArray -> hexadecimal
    )
    .fetch_optional(pool)
    .await
    .context("Failed to query db")
    .map_err(PublishError::UnexpectedError)?
    // note: the book just uses `map` to unpack the fields from within the `Some`, thus returning a
    // `Result<Option<(...)>>`. to streamline things, i use `map_err` (again) to convert `Option` to
    // `Result`, and lift the fields from `Some`
    .context("No user with the supplied username was found in users table")
    .map_err(PublishError::AuthError)?;
    Ok((row.user_id, Secret::new(row.password_hash)))
}

/// Note that verification is a CPU-bound operation that is fairly slow (by
/// design)
// up to 0.5 s (!)
// TEST_LOG=true cargo test confirmed | grep VERIF | bunyan
fn verify_password(
    supplied_password: Secret<String>,
    stored_password: Secret<String>,
) -> Result<(), PublishError> {
    let stored_password = &PasswordHash::new(stored_password.expose_secret())
        .context("Failed to read stored PHC string")
        .map_err(PublishError::UnexpectedError)?;
    Argon2::default()
        .verify_password(
            supplied_password.expose_secret().as_bytes(),
            stored_password,
        )
        .context("Invalid password")
        .map_err(PublishError::AuthError)?;
    Ok(())
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
/// Validate supplied credentials (username/password) by checking against the
/// `users` table in db, returning the user's `Uuid` on success.
#[tracing::instrument(name = "Validating credentials", skip(creds, pool))]
async fn validate_credentials(
    creds: Credentials,
    pool: &PgPool,
) -> Result<Uuid, PublishError> {
    // let (user_id, stored_password) = get_stored_credentials(creds.username,
    // pool).await?;

    let (user_id, stored_password) = match get_stored_credentials(creds.username, pool).await {
        Ok((i, p)) => (Some(i), p),
        // Notice that returning early here skips the (slow) hash verification, leading to a 10x
        // 'speedup'. This may be exploited for a timing attack, allowing attackers to
        // perform user enumeration and determine which usernames are valid (and which
        // aren't). To avoid this, use a fallback hash (which must be a valid PHC with the same
        // params; otherwise verification will also be quick) to ensure constant computation time
        // regardless of user validity.
        Err(_) => (
            None,
            Secret::new(
                // these argon2 params correspond with those declared in `TestUser.store`
                "$argon2id$v=19$m=19456,t=2,p=1$\
        gZiV/M1gPc22ElAH/Jh1Hw$\
        CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
                    .to_string(),
            ),
        ),
    };

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

    // "[a] future progresses from the previous .await to the next one and then
    // yields control back to the executor."
    //
    // "async runtimes make progress concurrently on multiple I/O-bound tasks by
    // continuously parking and resuming each of them." generally, any task that
    // takes more than 1 ms can be said to be CPU-bound, and should be handed
    // off to a separate threadpool (that does -not- yield)

    /// Wrapper for `spawn_blocking` with `tracing`
    pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let span = tracing::Span::current();

        tokio::task::spawn_blocking(move || {
            // tracing::info_span!("Verifying password hash").in_scope(|| {
            span.in_scope(
                // 1. `verify_password` strictly requires both args to be refs (`to_owned` won't
                //    work)
                // 2. `move`ing refs into a thread is forbidden by the borrow checker; a thread
                //    spawned by `spawn_blocking` is assumed to last for the duration of the entire
                //    program
                // 3. we want to be able catch `Err` from `PasswordHash::new`; this is not trivial
                //    from within a thread
                //
                // instead, only owned data should be moved into the thread

                // Argon2::default().verify_password(
                //     creds.password.expose_secret().as_bytes(),
                //     &PasswordHash::new(stored_password.expose_secret())
                //         .context("Failed to read stored PHC string")
                //         .map_err(PublishError::UnexpectedError)
                //         .unwrap(),
                // )
                f,
            )
        })
    }

    // notice that there are 2 closures: the function (`verify_password`) is first
    // placed in a tracing span, and this span is then placed in a blocking
    // thread
    spawn_blocking_with_tracing(move || verify_password(creds.password, stored_password))
        .await
        .context("Failed to spawn blocking thread")
        .map_err(PublishError::UnexpectedError)?
        .context("Invalid password")
        .map_err(PublishError::AuthError)?;

    // Ok(user_id)
    user_id.ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Invalid username")))
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
