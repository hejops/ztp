use actix_web::web;
use actix_web::HttpResponse;
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
    email_client
        .send_email(
            new_sub.email,
            "foo",
            &format!("confirm at {confirm_link}").to_owned(),
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

/// `POST`. `form` is raw HTML, which is ultimately deserialized, in order to
/// perform two SQL `INSERT` queries. Sends a confirmation email to the email
/// address passed by the user.
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
) -> HttpResponse {
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

    // let new_sub = match NewSubscriber::new(form.0.name, form.0.email) {
    // implementing either `TryFrom` or `TryInto` automatically implements the other
    // one for free; try_into() is generally preferred since it uses `.` instead
    // of `::`
    // let new_sub = match NewSubscriber::try_from(form.0) {
    let new_sub: NewSubscriber = match form.0.try_into() {
        // # Request parsing
        //
        // How `Form` -> `Result` extraction works: `FromRequest` trait provides the `from_request`
        // method, which takes `HttpRequest` + `Payload`, and implicitly 'wraps' the return value
        // as `Result<Self, Self::Error>` (in practice, this usually means (200, 400)).
        //
        // Under the hood, `from_request` uses `UrlEncoded::new`, and
        // `serde_urlencoded::from_bytes`.
        //
        // # Deserialization, serde
        //
        // `serde` defines a set of data models, agnostic to specific data formats like JSON.
        //
        // The `Serialize` trait (`serialize` method) converts a single type `T` (e.g. `Vec`) into
        // `Result`:
        //
        // ```rust,ignore
        //     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        // ```
        //
        // The `Serializer` trait (`serialize_X` methods) converts any and all arbitrary Rust types
        // `T` into `Result`.
        //
        // Monomorphisation is a zero-cost abstraction (no runtime cost). Proc macros
        // (`#[derive(Deserialize)]`) make parsing convenient.
        Ok(n) => n,
        // unfortunately we can't do ?-style early return/method chaining with HttpResponse
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    // println!("starting transaction");

    // if user requests `subscriptions` more than once, email and token should
    // already be present in dbs, so just send another email (with stored token)
    // and return early. this can be done before the transaction even begins
    if let Ok(Some(id)) = get_subscriber_id_from_email(&pool, &new_sub.email).await {
        let token = match get_subscriber_token(&pool, &id).await {
            Ok(Some(t)) => t,
            _ => return HttpResponse::InternalServerError().finish(),
        };
        match send_confirmation_email(&email_client, new_sub, &base_url.0, &token).await {
            Ok(_) => return HttpResponse::Ok().finish(),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        }
    };

    // this transaction groups 2 additions into 2 tables
    let mut transaction = match pool.begin().await {
        Ok(t) => t,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // coerce sqlx::Error into http 500
    let id = match insert_subscriber(
        &new_sub,
        // &pool,
        &mut transaction,
    )
    .await
    {
        // Ok(_) => (),
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // println!("{} {:?}", id, new_sub.email);
    // println!("storing token");

    let token: String = {
        let mut rng = thread_rng();
        (0..25).map(|_| rng.sample(Alphanumeric) as char).collect()
    };

    match store_token(
        // &pool,
        &mut transaction,
        id,
        &token,
    )
    .await
    {
        Ok(_) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // println!("storing token ok");

    match transaction.commit().await {
        Ok(_) => (),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // println!("transaction ok");

    match send_confirmation_email(&email_client, new_sub, &base_url.0, &token).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
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
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        "
    INSERT INTO subscription_tokens (subscriber_id, subscription_token)
    VALUES ($1, $2)
",
        id,
        token,
    );
    transaction.execute(query).await.map_err(|e| {
        tracing::error!("bad query: {e:?}");
        e
    })?;
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
