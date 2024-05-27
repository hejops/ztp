use std::fmt::Debug;

use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;
use secrecy::Secret;
use serde::Deserialize;
use sqlx::PgPool;

use crate::authentication::validate_credentials;
use crate::authentication::AuthError;
use crate::authentication::Credentials;
use crate::routes::error_chain_fmt;
use crate::session_state::TypedSession;

/// Login credentials
#[derive(Deserialize)]
pub struct LoginFormData {
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

// a stateless (client-only, db-free) extension to cookies are session tokens
// (JWTs): "Users are asked to authenticate once, via a login form: if
// successful, the server generates [...] an authenticated session token", which
// is attached to all subsequent requests
//
// https://developer.okta.com/blog/2022/02/08/cookies-vs-tokens#what-you-should-know-about-cookies
//
// tokens must be random, unique and short-lived, and sessions must store some
// state. while the short-lived nature of tokens is a poor fit for on-disk dbs
// (e.g. Postgres) as they have no concept of row expiration, it is a good fit
// for in-memory dbs (e.g. Redis)

// we always work on a single session at a time, identified using its token. the
// session token acts as the key, while the value is the JSON representation of
// the session state - the middleware takes care of (de)serialization.

/// `POST /login`
///
/// Triggered after submitting valid credentials on `/login`.
///
/// On successful validation, `GET /admin/dashboard`, otherwise `GET /login`
/// again with error message (and HMAC tag) injected as params.
// note: since authentication is done entirely via url, and we don't store/record the login in any
// meaningful way, "logging in" and revisiting the page with any params will still produce the same
// error message. instead of messing with the url, this should be done by cookies which are issued
// to clients
#[tracing::instrument(
    name = "Validating credentials for login",
    skip(form, pool, session),
    fields(
        username=tracing::field::Empty,
        user_id=tracing::field::Empty,
    )
)]
pub async fn login(
    form: web::Form<LoginFormData>,
    pool: web::Data<PgPool>,
    // secret: web::Data<Secret<String>>,
    // secret: web::Data<HmacSecret>,
    // session: Session,
    session: TypedSession,
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

    fn login_redirect(err: LoginError) -> InternalError<LoginError> {
        FlashMessage::error(err.to_string()).send();
        let resp = HttpResponse::SeeOther()
            .insert_header(("LOCATION", "/login"))
            .finish();
        // InternalError::new(err, resp.status())
        // equivalent
        InternalError::from_response(err, resp)
    }

    match validate_credentials(creds, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(user_id));

            // clear session to mitigate session fixation
            // https://en.wikipedia.org/wiki/Session_fixation
            // https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#renew-the-session-id-after-any-privilege-level-change
            session.renew();
            // session state is implicitly stored in redis when the response is returned
            session
                // .insert("user_id", user_id)
                .insert_user_id(user_id)
                .map_err(|e| login_redirect(LoginError::UnexpectedError(e.into())))?;

            Ok(
                // 303
                HttpResponse::SeeOther() // https://developer.mozilla.org/en-US/docs/Web/HTTP/Redirections#temporary_redirections
                    .insert_header((LOCATION, "/admin/dashboard")) // replace the location, i.e. redirect
                    .finish(),
            )
        }

        Err(e) => {
            let e = match e {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
            };

            // we will soon move this from url params to cookie header
            // let encoded_error = urlencoding::Encoded::new(e.to_string());
            // let error = format!("error_msg={encoded_error}");

            // let secret = secret.0.expose_secret().as_bytes();
            // // byte slice encoded as hex string; this must be decoded on reload
            // let hmac_tag = {
            //     let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).unwrap();
            //     mac.update(error.as_bytes());
            //     mac.finalize().into_bytes()
            // };

            // http://localhost:8000/login?error=You%20are%20not...&tag=dfe219b336b...
            // let location = format!("/login?{error}&tag={hmac_tag:x}");
            // let location = "/login".to_owned();

            // "Session cookies are stored in memory - they are deleted when the session
            // ends (i.e. the browser is closed). Persistent cookies, instead,
            // are saved to disk and will still be there when you re-open the
            // browser."

            // let resp = HttpResponse::SeeOther()
            //     .insert_header((LOCATION, location))
            //     // .insert_header(("Set-Cookie", format!("_flash={e}")))
            //     // .cookie(Cookie::new("_flash", e.to_string()))
            //     .finish();
            //
            // // supersedes manual setting of cookie!
            // FlashMessage::error(e.to_string()).send();
            //
            // Err(InternalError::from_response(e, resp))

            Err(login_redirect(e))
        }
    }
}
