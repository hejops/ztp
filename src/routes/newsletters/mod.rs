mod get;
mod post;
use std::fmt::Debug;

use actix_web::http::header;
use actix_web::http::header::HeaderValue;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use actix_web::ResponseError;
pub use get::*;
pub use post::*;
use serde::Deserialize;

use super::error_chain_fmt;
use crate::domain::SubscriberEmail;

#[derive(Deserialize)]
pub struct Newsletter {
    title: String,
    // content: NewsletterContent,
    content: String,
}

// #[derive(Deserialize)]
// struct NewsletterContent {
//     html: String,
//     text: String,
// }

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

// Parse headers of a HTTP request. This does not actually validate any user
// credentials; for that, see `validate_credentials`.
// fn basic_authentication(headers: &HeaderMap) -> Result<Credentials,
// anyhow::Error> {     // authentication methods fall in three categories:
// passwords / objects /     // biometrics. because there are drawbacks
// associated with each, multi-factor     // authentication is recommended
//
//     // spec: RFCs 2617, 7617
//     // - correct header ("Authorization")
//     // - correct realm ("publish")
//     // - correct username/password
//
//     let encoded = headers
//         .get("Authorization")
//         .context("No Authorization header")?
//         .to_str()
//         .context("Invalid str")?
//         .strip_prefix("Basic ")
//         .context("Authorization scheme was not 'Basic'")?;
//
//     let decoded = base64::engine::general_purpose::STANDARD
//         .decode(encoded)
//         .context("Failed to decode base64")?;
//     let decoded = String::from_utf8(decoded).context("Invalid str")?;
//
//     let mut creds = decoded.splitn(2, ':');
//
//     let username = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No username"))?
//         .to_string();
//
//     let password = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No password"))?
//         .to_string();
//     let password = Secret::new(password);
//
//     Ok(Credentials { username, password })
// }
