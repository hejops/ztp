use actix_web::web;
use actix_web::HttpResponse;
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

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
    // query is statically checked against db schema (migrations/xxx.sql or
    // postgres?) at compile time
    // (if 'relation does not exist', restart LSP)

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
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("bad query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
