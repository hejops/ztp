use actix_web::web::Form;
use actix_web::HttpResponse;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct FormData {
    name: String,
    email: String,
}

/// If invalid data is encountered, the function returns early, and is
/// transformed into an `Error` (400) automatically, even if `form` is not
/// used.
///
/// Note: if the function takes no arguments, it will always return 200,
/// even on invalid data.
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
pub async fn subscribe(form: Form<FormData>) -> HttpResponse { HttpResponse::Ok().finish() }
