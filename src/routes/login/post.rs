use std::fmt::Debug;

use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::HttpResponse;
use hmac::Hmac;
use hmac::Mac;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;

use crate::authentication::validate_credentials;
use crate::authentication::AuthError;
use crate::authentication::Credentials;
use crate::routes::error_chain_fmt;
use crate::startup::HmacSecret;

/// Login credentials
#[derive(Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

/// Derived from `PublishError` (which was written first)
#[derive(thiserror::Error)]
pub enum LoginError {
    // this error string will be displayed in the browser
    #[error("You are not authorized to view this page.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl Debug for LoginError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        error_chain_fmt(self, f)?;
        Ok(())
    }
}

/// `POST` endpoint (`login`)
///
/// Triggered after submitting valid credentials on `/login`.
///
/// On successful validation, `GET /`, otherwise `GET /login` again with
/// error message (and HMAC tag) injected as params.
#[tracing::instrument(
    name = "Validating credentials for login",
    skip(form, pool, secret),
    fields(
        username=tracing::field::Empty,
        user_id=tracing::field::Empty,
    )
)]
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    // secret: web::Data<Secret<String>>,
    secret: web::Data<HmacSecret>,
    // returning `Err(impl ResponseError)` is required for graceful exit
    // ) -> Result<HttpResponse, LoginError> {
    // ) -> HttpResponse {
    //
    // `InternalError` combines `ResponseError` (thus propagating the error context upstream to the
    // middleware chain on failure) and `HttpResponse` (triggering the correct redirects on both
    // success and failure).
) -> Result<HttpResponse, InternalError<LoginError>> {
    let creds = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    tracing::Span::current().record("username", tracing::field::display(&creds.username));

    // previously, we just returned early on validation failure, without causing a
    // reload (/error message)

    // let user_id = validate_credentials(creds, &pool)
    //     .await
    //     .map_err(|e| match e {
    //         AuthError::UnexpectedError(e) => LoginError::UnexpectedError(e),
    //         AuthError::InvalidCredentials(e) => LoginError::AuthError(e),
    //     })?;
    //
    // tracing::Span::current().record("user_id", tracing::field::display(user_id));
    //
    // Ok(
    //     HttpResponse::SeeOther() // https://developer.mozilla.org/en-US/docs/Web/HTTP/Redirections#temporary_redirections
    //         // replace the location with / (home), i.e. redirect
    //         .insert_header((LOCATION, "/"))
    //         .finish(),
    // );

    match validate_credentials(creds, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(user_id));

            Ok(
                HttpResponse::SeeOther() // https://developer.mozilla.org/en-US/docs/Web/HTTP/Redirections#temporary_redirections
                    .insert_header((LOCATION, "/")) // replace the location with / (home), i.e. redirect
                    .finish(),
            )
        }
        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };
            let encoded_error = urlencoding::Encoded::new(e.to_string());
            let error = format!("error_msg={encoded_error}");

            let secret = secret.0.expose_secret().as_bytes();
            // byte slice encoded as hex string; this must be decoded on reload
            let hmac_tag = {
                let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).unwrap();
                mac.update(error.as_bytes());
                mac.finalize().into_bytes()
            };

            // http://localhost:8000/login?error=You%20are%20not...&tag=dfe219b336b...

            let resp = HttpResponse::SeeOther()
                .insert_header((LOCATION, format!("/login?{error}&tag={hmac_tag:x}")))
                .finish();

            Err(InternalError::from_response(e, resp))
        }
    }
}
