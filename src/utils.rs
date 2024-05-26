use std::fmt::Debug;
use std::fmt::Display;

use actix_web::http::header::LOCATION;
use actix_web::HttpResponse;

/// Convert arbitrary error types to `actix_web::Error` with HTTP 500
pub fn error_500<T>(e: T) -> actix_web::Error
where
    T: Debug + Display + 'static,
{
    actix_web::error::ErrorInternalServerError(e)
}

pub fn redirect(location: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header((LOCATION, location))
        .finish()
}
