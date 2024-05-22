use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use hmac::Hmac;
use hmac::Mac;
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::startup::HmacSecret;

#[derive(Deserialize)]
pub struct QueryParams {
    /// In the interest of clarity, `error_msg` is used over `error`
    error_msg: String,

    /// Byte slice encoded as hex string
    tag: String,
}

impl QueryParams {
    /// Construct a HMAC instance and use it to verify `secret`
    fn verify(
        self,
        supplied_secret: &HmacSecret,
    ) -> Result<String, anyhow::Error> {
        let tag = hex::decode(self.tag)?;
        // see `login`
        let encoded_error = urlencoding::Encoded::new(&self.error_msg);
        let query_string = format!("error_msg={encoded_error}");

        let mut mac =
            Hmac::<sha2::Sha256>::new_from_slice(supplied_secret.0.expose_secret().as_bytes())?;
        mac.update(query_string.as_bytes());
        mac.verify_slice(&tag)?;
        Ok(self.error_msg)
    }
}

/// `GET` endpoint (`login`)
///
/// Requested with empty `query` by default (zero params), but may be requested
/// (via redirect) with exactly two params (`error`, `tag`). HMAC prevents this
/// page from being loaded with arbitrary params.
// GET login -> enter creds -> POST login --> ok -> /
// ^------------------ not ok -/
pub async fn login_form(
    query: Option<web::Query<QueryParams>>,
    secret: web::Data<HmacSecret>,
) -> HttpResponse {
    let error_msg = match query {
        // no params, or failed to deserialize, e.g. http://localhost:8000/login?error_msg=foo
        None => "".to_owned(),
        Some(query) => {
            match query.0.verify(&secret) {
                // warning: injecting query params like this easily opens the door to XSS;
                // mitigate this by escaping html, and hmac
                // https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
                // valid params (hmac hash matches what we expect)
                Ok(error_msg) => {
                    format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error_msg))
                }
                // malformed params; just reload the page (with a different error msg)
                Err(e) => {
                    tracing::warn!(
                            error.message = %e,
                            error.cause_chain = ?e,
                            "HMAC verification failed",
                    );
                    format!(
                        "<p><i>{}</i></p>",
                        htmlescape::encode_minimal("URL parameters have been tampered with!")
                    )
                }
            }
        }
    };

    // let body = include_str!("./login.html"); // static html

    let body = format!(
        r#"
<!doctype html>
<html lang="en">
  <head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8" />
    <title>Login</title>
  </head>
  <body>
    {error_msg}
    <!-- trigger `POST` request to `/login` on submit; otherwise, credentials will be put in URL! -->
    <!-- e.g. http://localhost:8000/login?username=foo&password=bar -->
    <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/form#action -->
    <form action="/login" method="post">
      <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/label -->
      <label>
        Username
        <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Input -->
        <input type="text" placeholder="Enter Username" name="username" />
      </label>
      <label>
        Password
        <input type="password" placeholder="Enter Password" name="password" />
      </label>
      <button type="submit">Login</button>
    </form>
  </body>
</html>
    "#,
    );

    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body)
}
