use actix_web::web;
use actix_web::HttpResponse;
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::Instrument;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

/// # Request example
///
/// ```sh
///     curl -i -X POST -d 'email=john@foo.com&name=John' http://127.0.0.1:8000/subscriptions
/// ```
///
/// # Arguments
///
/// `form` is passed as a raw HTTP request. Upon deserialization into our
/// `FormData` struct (via `Form` and `serde`), invalid data causes the function
/// to return early, returning an `Error` (400) automatically. Otherwise, the
/// successfully parsed request is added to the db.
///
/// (Note: if the function takes no arguments, it will always return 200,
/// even on invalid data.)
///
/// `PgPool` is used over `PgConnection` as the former has has `Mutex`
/// 'built-in'.
// "when you run a query against a `&PgPool`, `sqlx` will borrow a `PgConnection` from the pool and
// use it to execute the query; if no connection is available, it will create a new one or wait
// until one frees up."
///
/// # Request parsing
///
/// How `Form` -> `Result` extraction works: `FromRequest` trait provides
/// the `from_request` method, which takes `HttpRequest` + `Payload`,
/// and implicitly 'wraps' the return value as `Result<Self, Self::Error>`
/// (in practice, this usually means (200, 400)).
///
/// Under the hood, `from_request` uses `UrlEncoded::new`, and
/// `serde_urlencoded::from_bytes`.
///
/// # Deserialization, serde
///
/// `serde` defines a set of data models, agnostic to specific data formats
/// like JSON.
///
/// The `Serialize` trait (`serialize` method) converts a single type `T`
/// (e.g. `Vec`) into `Result`:
///
/// ```rust,ignore
///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
/// ```
///
/// The `Serializer` trait (`serialize_X` methods) converts any and all
/// arbitrary Rust types `T` into `Result`.
///
/// Monomorphisation is a zero-cost abstraction (no runtime cost). Proc
/// macros (`#[derive(Deserialize)]`) make parsing convenient.
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    let id = Uuid::new_v4();

    let req_span = tracing::info_span!(
        // with `log` feature, tracing events are redirected to `log` automatically
        // note: formatting is disabled in this macro!
        "Adding new subscriber",
        %id, // equivalent to `id = %id`
        subscriber_email = %form.email, // named key
        subscriber_name = %form.name,
    );
    let _enter = req_span.enter(); // this span is sync

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
    // when a future (task) is idle, the executor may switch to a different task.
    // however, the span would be unaware of this switch, and would (sort of) lead
    // to the interleaving we wanted to avoid in the first place. to correctly
    // switch spans, use `tracing::Instrument` and attach the span to the async fn
    let query_span = tracing::info_span!("INSERTing new subscriber into db");

    // query is statically checked against db schema at compile time, but postgres
    // must be running
    //
    // (if 'relation does not exist', start postgres and restart LSP)
    // TODO: error if postgres not started, should be caught
    match sqlx::query!(
        "
    INSERT INTO subscriptions (id, email, name, subscribed_at)
    VALUES ($1, $2, $3, $4)
",
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(),
    )
    // `Executor` requires mut ref (sqlx's async does not imply mutex). PgPool handles this, but
    // PgConnection doesn't
    .execute(pool.get_ref())
    .instrument(query_span)
    .await
    {
        Ok(_) => {
            tracing::info!("Added new subscriber to db");
            HttpResponse::Ok().finish()
        }
        Err(e) => {
            // note: this is -not- covered by the span!
            tracing::error!("bad query: {e:?}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
