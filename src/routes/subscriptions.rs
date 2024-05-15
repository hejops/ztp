use std::fmt::Debug;
use std::fmt::Display;

use actix_web::http::StatusCode;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web::ResponseError;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use serde::Deserialize;
use sqlx::Executor;
use sqlx::PgPool;
use sqlx::Postgres;
use sqlx::Transaction;
use uuid::Uuid;

use crate::domain::NewSubscriber;
use crate::domain::SubscriberEmail;
use crate::domain::SubscriberName;
use crate::email_client::EmailClient;
use crate::startup::AppBaseUrl;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

// personally i would've placed this in `new_subscriber` (since i like to keep
// structs and impls together), but this requires `FormData`'s fields to be
// `pub`
impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;

        let new_sub = NewSubscriber {
            // // can be done if the field is `pub` (which it isn't)
            // name: SubscriberName(name.clone()),
            // // `.0` is required to access the fields in `FormData` (this is not documented in
            // // `Form` apparently)
            // name: SubscriberName::parse(name).unwrap(),
            name,
            email,
        };
        Ok(new_sub)
    }
}

// validation is inherently not robust, because, in the worst case, it has to be
// performed at every callsite. importantly, validation is performed at runtime,
// so the compiler will -not- catch validation errors.
//
// in contrast, parsing can be done just once to transform unstructured data
// into a structured representation (i.e. a struct), which can then be passed
// around with confidence in its correctness, due to compile-time checks.

// after email validation, it is still necessary to confirm user consent with a
// confirmation email

/// Wrapper for `EmailClient.send_email`. Probably should be declared here and
/// left private (rather than a public `EmailClient.send_confirmation_email`
/// method).
#[tracing::instrument(
    name = "Sending confirmation email to new subscriber",
    skip(email_client, new_sub, base_url, token)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    new_sub: NewSubscriber,
    base_url: &str,
    token: &str,
) -> Result<(), reqwest::Error> {
    let confirm_link = format!("{base_url}/subscriptions/confirm?subscription_token={token}");
    println!("sending email to {:?}", new_sub.email);

    // https://keats.github.io/tera/docs/#base-template
    // https://github.com/Keats/tera/blob/3b2e96f624bd898cc96e964cd63194d58701ca4a/benches/templates.rs#L45

    use tera::Context;
    use tera::Tera;

    let template = r#"<!doctype html>
<html lang="en">
  <head>
    <title>{{ title }}</title>
  </head>
  <body>
    <h1>You're confirmed!</h1>
    <div id="content">
      Hello, {{ name }}. To confirm your subscription, click
      <a href="{{ link }}">here</a>.
    </div>
  </body>
</html>"#;

    let mut tera = Tera::default();
    tera.autoescape_on(vec![]); // don't escape confirm_link
    tera.add_raw_templates(vec![("confirm.html", template)])
        .unwrap();

    let mut context = Context::new();
    context.insert("title", "Confirm your subscription");
    context.insert("name", new_sub.name.as_ref());
    context.insert("link", &confirm_link);

    let html = tera.render("confirm.html", &context).unwrap();

    email_client
        .send_email(
            new_sub.email,
            "foo",
            // &format!("confirm at {confirm_link}").to_owned(),
            &html,
            &format!("confirm at {confirm_link}").to_owned(),
        )
        .await
}

/// Fails if `email` not found in `subscriptions` table. The `id` returned may
/// be empty, so this should be checked by the caller.
///
/// (extra function written beyond the scope of the book)
#[tracing::instrument(name = "Getting email of subscriber", skip(pool, email))]
pub async fn get_subscriber_id_from_email(
    pool: &PgPool,
    email: &SubscriberEmail,
) -> Result<Option<Uuid>, sqlx::Error> {
    let id = sqlx::query!(
        "
    SELECT id FROM subscriptions
    WHERE email = $1
",
        email.as_ref(),
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("bad query: {e:?}");
        e
    })?
    .map(|u| u.id);
    Ok(id)
}

/// (extra function written beyond the scope of the book)
#[tracing::instrument(name = "Getting token of subscriber", skip(pool, id))]
pub async fn get_subscriber_token(
    pool: &PgPool,
    id: &Uuid,
) -> Result<Option<String>, sqlx::Error> {
    let id = sqlx::query!(
        "
    SELECT subscription_token FROM subscription_tokens
    WHERE subscriber_id = $1
",
        id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("bad query: {e:?}");
        e
    })?
    .map(|u| u.subscription_token);
    Ok(id)
}

/// Print a complete error chain recursively
fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{e}\n")?;
    let mut src = e.source();
    while let Some(cause) = src {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        src = cause.source();
    }
    Ok(())
}

// so far we haven't distinguished between failure modes (enum variants) if more
// than one is possible; sqlx::Error, for example, has many failure modes.
//
// inform the caller how to react
// inform the operator how to troubleshoot
// inform the user how to troubleshoot (only necessary when user can actually
// take steps to correct the error)
//
// internal control flow: types, methods, fields
// external control flow: status codes (http)
// internal reporting: logging, tracing
// external reporting: response body

/// A wrapper type that allows various error types (e.g. `sqlx::Error`) to be
/// coerced into `actix_web::Error`, with varying http codes (typically 500, but
/// sometimes 400).
// #[derive(Debug)]
//
pub enum SubscribeError {
    ValidationError(String),
    SendEmailError(reqwest::Error),

    // DatabaseError(sqlx::Error),
    CommitTransactionError(sqlx::Error),
    InsertSubscriberError(sqlx::Error),
    PoolError(sqlx::Error),
    StoreTokenError(sqlx::Error),
}

// `impl From<T> for X` enables automatic wrapping of `T` in one variant of `X`,
// and thus ?. however, we cannot use `From` if `T` can be wrapped by multiple
// variants of X.
impl From<String> for SubscribeError {
    fn from(value: String) -> Self { Self::ValidationError(value) }
}
impl From<reqwest::Error> for SubscribeError {
    fn from(value: reqwest::Error) -> Self { Self::SendEmailError(value) }
}

// for any Error to be wrapped, -both- `Debug` and `Display` must be
// implemented. with the default `Debug` impl, the trace and exception.details
// run on, with no clear visual separation, and `exception.details` will use
// `Debug` (instead of the more concise `Display`).
impl Debug for SubscribeError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        error_chain_fmt(self, f)?;
        Ok(())
    }
}

impl Display for SubscribeError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        // write!(f, "Failed to create subscriber")?;
        match self {
            Self::CommitTransactionError(_) => write!(f, "Failed to commit transaction"),
            Self::InsertSubscriberError(_) => write!(f, "Failed to insert subscriber"),
            Self::PoolError(_) => write!(f, "Failed to connect to db pool"),
            Self::SendEmailError(_) => write!(f, "Failed to send confirmation email"),
            Self::StoreTokenError(_) => write!(f, "Failed to store token"),
            Self::ValidationError(e) => write!(f, "{e}"),
        }
    }
}

// enable automatic conversion into actix_web::Error
impl ResponseError for SubscribeError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            Self::ValidationError(_) => StatusCode::BAD_REQUEST, // 400
            _ => StatusCode::INTERNAL_SERVER_ERROR,              // 500
        }
    }
}

impl std::error::Error for SubscribeError {
    // "`source` is useful when writing code that needs to handle a variety of
    // errors: it provides a structured way to navigate the error chain without
    // having to know anything about the specific error type you are working
    // with."
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Some(&self.0)
        match self {
            Self::ValidationError(_) => None,

            Self::CommitTransactionError(e) => Some(e),
            Self::InsertSubscriberError(e) => Some(e),
            Self::PoolError(e) => Some(e),
            Self::SendEmailError(e) => Some(e),
            Self::StoreTokenError(e) => Some(e),
        }
    }
}

/// `POST` endpoint (`subscribe`)
///
/// `form` is raw HTML, which is ultimately deserialized, in order to perform
/// two SQL `INSERT` queries. Sends a confirmation email to the email address
/// passed by the user.
///
/// Success requires:
///     1. user input parsed
///     2. user added to db AND user token added to db (transaction)
///     3. email sent to user email
///
/// Clients are expected to call `subscriptions/confirm` next.
///
/// # Request example
///
/// ```sh
///     curl -v --include --data 'email=john@foo.com&name=John' http://127.0.0.1:8000/subscriptions
///     curl --data 'email=john@foo.com&name=John' http://127.0.0.1:8000/subscriptions
/// ```
///
/// # Arguments
///
/// `form` is passed as a raw HTTP request. Upon deserialization into our
/// `FormData` struct (via `Form` and `serde`), invalid data causes the function
/// to return early, returning an `Error` (400) automatically. Otherwise, the
/// successfully parsed request is added to the db.
///
/// All other args are implicity passed via `.app_data`
// (Note: if the function takes no arguments, it will always return 200,
// even on invalid data.)
///
/// `PgPool` is used over `PgConnection` as the former has has `Mutex`
/// 'built-in'.
// "when you run a query against a `&PgPool`, `sqlx` will borrow a `PgConnection` from the pool and
// use it to execute the query; if no connection is available, it will create a new one or wait
// until one frees up."
#[tracing::instrument(
    // to separate instrumentation (tracing) from execution (i.e. the actual work, in this
    // case`sqlx::query`), the entire function is wrapped in a span. note that the return value is
    // wrapped by `tracing`
    name = "Adding new subscriber", // defaults to fn name
    // don't log passed args
    skip(form, pool, email_client, base_url),
    fields(
        // same syntax as info_span
        // should not be used in conjunction with TracingLogger, as TracingLogger generates its own ids
        // id = %Uuid::new_v4(), 
        subscriber_email = %form.email,
        subscriber_name = %form.name,
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    // all subsequent args are inherited via App.app_data; thus arg types must be unique
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<AppBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    // // with `log` feature, tracing events are redirected to `log`
    // // automatically
    // let id = Uuid::new_v4();
    // let req_span = tracing::info_span!(
    // // note: formatting is disabled in this macro!
    //     "Adding new subscriber",
    //     %id, // equivalent to `id = %id`
    //     subscriber_email = %form.email, // named key
    //     subscriber_name = %form.name,
    // );
    // let _enter = req_span.enter(); // this span is sync

    // the span persists until the end of the function, where it is dropped
    //
    // -> entered span
    // <- exited span
    // -- closed span (drop)

    // .enter should not be used in an async fn; from method docs:
    //
    // "...[an] `await` keyword may yield, causing the runtime to switch to
    // another task, while remaining in this span!"
    //
    // when a future (task) is idle, the executor may switch to a different
    // task. however, the span would be unaware of this switch, and would
    // (sort of) lead to the interleaving we wanted to avoid in the first
    // place. to correctly switch spans, use `tracing::Instrument` and
    // attach the span to the async fn

    // // naive string validation
    // if !is_valid_name(&form.name) {
    //     return HttpResponse::BadRequest().finish();
    // }

    // # Request parsing
    //
    // How `Form` -> `Result` extraction works: `FromRequest` trait provides the
    // `from_request` method, which takes `HttpRequest` + `Payload`, and
    // implicitly 'wraps' the return value as `Result<Self, Self::Error>` (in
    // practice, this usually means (200, 400)).
    //
    // Under the hood, `from_request` uses `UrlEncoded::new`, and
    // `serde_urlencoded::from_bytes`.
    //
    // # Deserialization, serde
    //
    // `serde` defines a set of data models, agnostic to specific data formats like
    // JSON.
    //
    // The `Serialize` trait (`serialize` method) converts a single type `T` (e.g.
    // `Vec`) into `Result`:
    //
    // ```rust,ignore
    //     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    // ```
    //
    // The `Serializer` trait (`serialize_X` methods) converts any and all arbitrary
    // Rust types `T` into `Result`.
    //
    // Monomorphisation is a zero-cost abstraction (no runtime cost). Proc macros
    // (`#[derive(Deserialize)]`) make parsing convenient.

    // let new_sub = match NewSubscriber::new(form.0.name, form.0.email) {
    // implementing either `TryFrom` or `TryInto` automatically implements the other
    // one for free; try_into() is generally preferred since it uses `.` instead
    // of `::`
    // let new_sub = match NewSubscriber::try_from(form.0) {
    let new_sub: NewSubscriber = form.0.try_into()?;
    // {
    //     Ok(n) => n,
    //     // unfortunately we can't do ?-style early return/method chaining with
    // HttpResponse     Err(_) => return
    // Ok(HttpResponse::BadRequest().finish()), };

    // println!("starting transaction");

    // extra: if user requests `subscriptions` more than once, email and token
    // should already be present in dbs, so just send another email (with stored
    // token) and return early. this can be done before the transaction even
    // begins
    if let Ok(Some(id)) = get_subscriber_id_from_email(&pool, &new_sub.email).await {
        if let Ok(Some(token)) = get_subscriber_token(&pool, &id).await {
            return Ok(
                match send_confirmation_email(&email_client, new_sub, &base_url.0, &token).await {
                    Ok(_) => HttpResponse::Ok().finish(),
                    Err(_) => HttpResponse::InternalServerError().finish(),
                },
            );
        };
    };

    // this transaction groups 2 additions into 2 tables
    // wrap sqlx::Error in our own wrapper type, allowing early return with ?
    let mut transaction = pool.begin().await.map_err(SubscribeError::PoolError)?;

    let id = insert_subscriber(&new_sub, &mut transaction)
        .await
        // map_err is required since our function returns generic sqlx::Error; this may be changed
        // soon
        .map_err(SubscribeError::InsertSubscriberError)?;

    // println!("{} {:?}", id, new_sub.email);
    // println!("storing token");

    let token: String = {
        let mut rng = thread_rng();
        (0..25).map(|_| rng.sample(Alphanumeric) as char).collect()
    };

    // map_err is not needed because the function already returns a SubscribeError
    store_token(&mut transaction, id, &token).await?;

    // println!("storing token ok");

    transaction
        .commit()
        .await
        .map_err(SubscribeError::CommitTransactionError)?;

    // println!("transaction ok");

    // we don't need map_err here; implementing `From` automagically enables ?
    send_confirmation_email(&email_client, new_sub, &base_url.0, &token).await?;

    Ok(HttpResponse::Ok().finish())
}

/// Add randomly generated `token` to `subscription_tokens` table
#[tracing::instrument(
    name = "INSERTing new subscriber token into subscription_tokens table",
    skip(transaction, token)
)]
async fn store_token(
    // pool: &PgPool,
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
    token: &str,
) -> Result<
    (),
    // sqlx::Error,
    SubscribeError,
> {
    let query = sqlx::query!(
        "
    INSERT INTO subscription_tokens (subscriber_id, subscription_token)
    VALUES ($1, $2)
",
        id,
        token,
    );
    transaction
        .execute(query)
        .await
        .map_err(SubscribeError::StoreTokenError)?;
    Ok(())
}

/// Assign unique identifier to new user, add user to `subscriptions` table, and
/// return the identifier for subsequent confirmation (see
/// `subscriptions/confirm`).
///
/// Fails if user email has already been added to `subscriptions` table.
///
/// Only db logic is performed here; i.e. this is independent of web framework.
///
/// `sqlx::query!` can validate fields at compile time, but this requires
/// - a `DATABASE_URL` env var declared (typically in `./.env`), and a running
///   db (online mode)
/// - a `SQLX_OFFLINE` env var set to true, and a `.sqlx` directory, generated
///   by `cargo sqlx prepare --workspace`, which, in turn, also requires a
///   running db (offline mode)
// see:
// https://github.com/launchbadge/sqlx/blob/main/sqlx-cli/README.md#enable-building-in-offline-mode-with-query
// https://github.com/launchbadge/sqlx/blob/5d6c33ed65cc2d4671a9f569c565ab18f1ea67aa/sqlx-cli/src/prepare.rs#L65
///
/// Notes:
/// - functions marked as `test` are not subject to these compile-time checks
/// - conversely, `test` functions cannot be aware of offline mode
#[tracing::instrument(name = "INSERTing new subscriber into db", skip(new_sub, transaction))]
async fn insert_subscriber(
    // form: &FormData,
    new_sub: &NewSubscriber,
    // pool: &PgPool,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<Uuid, sqlx::Error> {
    // let query_span = tracing::info_span!("INSERTing new subscriber into db");

    // general threats to protect against include: SQL injection, denial of service,
    // data theft, phishing. it is not necessary to deal with all of these at
    // the outset, but it is good to keep them in mind

    // on db schema updates:
    //
    // we now need to add a new key to the db schema (status (str/enum)), and a new
    // table for subscription_token (uuid). this is a breaking change that must
    // be implemented with zero downtime.
    //
    // switching off instances of the old version and starting instances of the new
    // version will incur downtime.
    //
    // load balancers allow different versions of a server to coexist. this enables:
    // horizontal scaling, self-healing, rolling updates.
    //
    // AA -> AAB -> ABB -> BBB
    //
    // the server itself should be stateless; all state should be stored in the db,
    // which is the same across all server instances. thus, when the server api
    // changes, the db needs to support both the old and new versions:
    //
    // "if we want to evolve the database schema we cannot change the application
    // behaviour at the same time."
    //
    // new key: first add a migration to add the key as optional (NULL), preferably
    // with default value, then a separate migration to backfill and make the key
    // mandatory (NOT NULL)
    //
    // new table: just add the new migration

    let id = Uuid::new_v4();

    // note the difference in syntax:
    // query!().execute(pool) -> transaction.execute(query)

    let query = sqlx::query!(
        //         "
        //     INSERT INTO subscriptions (id, email, name, subscribed_at)
        //     VALUES ($1, $2, $3, $4)
        // ",
        "
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending_confirmation')
",
        id,
        new_sub.email.as_ref(),
        new_sub.name.as_ref(),
        Utc::now(),
    );
    // `Executor` requires mut ref (`sqlx`'s async does not imply mutex). `PgPool`
    // implements this, but `PgConnection` and `Transaction` don't
    transaction
        .execute(query)
        // .instrument(query_span)
        .await
        .map_err(|e| {
            tracing::error!("bad query: {e:?}");
            e
        })?;
    Ok(id)
}
