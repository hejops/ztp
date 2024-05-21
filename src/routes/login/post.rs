use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::HttpResponse;
use secrecy::Secret;
use serde::Deserialize;

/// Login credentials
#[derive(Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

/// `POST` endpoint (`login`)
///
/// Triggered after submitting valid credentials on `/login`
pub async fn login(form: web::Form<FormData>) -> HttpResponse {
    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Redirections#temporary_redirections
    HttpResponse::SeeOther()
        // replace the location with / (home), i.e. redirect
        .insert_header((LOCATION, "/"))
        .finish()
}
