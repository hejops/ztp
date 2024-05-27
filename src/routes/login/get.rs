use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;

// #[derive(Deserialize)]
// pub struct QueryParams {
//     /// In the interest of clarity, `error_msg` is used over `error`
//     error_msg: String,
//
//     /// Byte slice encoded as hex string
//     tag: String,
// }
//
// impl QueryParams {
//     /// Construct a HMAC instance and use it to verify `secret`
//     fn verify(
//         self,
//         supplied_secret: &HmacSecret,
//     ) -> Result<String, anyhow::Error> {
//         let tag = hex::decode(self.tag)?;
//         // see `login`
//         let encoded_error = urlencoding::Encoded::new(&self.error_msg);
//         let query_string = format!("error_msg={encoded_error}");
//
//         let mut mac =
//
// Hmac::<sha2::Sha256>::new_from_slice(supplied_secret.0.expose_secret().
// as_bytes())?;         mac.update(query_string.as_bytes());
//         mac.verify_slice(&tag)?;
//         Ok(self.error_msg)
//     }
// }

/// `GET /login`
///
/// Requested with empty `query` by default (zero params), but may be requested
/// (via redirect) with exactly two params (`error`, `tag`).
///
/// HMAC prevents this page from being loaded with arbitrary params.
// GET login -> enter creds -> POST login --> ok -> /
// ^------------------ not ok -/
pub async fn login_form(
    // query: Option<web::Query<QueryParams>>,
    // secret: web::Data<HmacSecret>,
    // request: HttpRequest,
    flash_messages: IncomingFlashMessages,
) -> HttpResponse {
    // let error_msg = match query {
    //     // no params, or failed to deserialize, e.g. http://localhost:8000/login?error_msg=foo
    //     None => "".to_owned(),
    //     Some(query) => {
    //         let error_msg = match query.0.verify(&secret) {
    //             // valid params (hmac hash matches what we expect)
    //             Ok(error_msg) => error_msg,
    //             // malformed params; just reload the page (with a different error
    //             // msg)
    //             Err(e) => {
    //                 tracing::warn!(
    //                         error.message = %e,
    //                         error.cause_chain = ?e,
    //                         "HMAC verification failed",
    //                 );
    //                 "URL parameters have been tampered with!".to_owned()
    //             }
    //         };
    //         // warning: injecting query params like this easily opens the door
    //         // to XSS;
    //         // mitigate this by escaping html, and hmac
    //         // https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
    //         format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error_msg))
    //     }
    // };

    // let body = include_str!("./login.html"); // static html

    // previously, the outcome of `POST /login` was stored in url params, which were
    // then decoded here. this becomes unnecessary if the outcome is stored in a
    // cookie
    //
    // generally, it is harder to perform XSS via cookies, but we must still guard
    // against tampering, sniffing and JavaScript (lol)

    // let error_msg = match request.cookie("_flash") {
    //     None => "".to_owned(),
    //     Some(error_msg) => format!("<p><i>{}</i></p>", error_msg.value()),
    // };

    let mut error_msg = String::new();
    for msg in flash_messages.iter() {
        //.filter(|m| m.level() == Level::Error) {
        // the book calls `writeln!(String)`, which is no longer allowed?
        // writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
        error_msg.push_str(&format!("<p><i>{}</i></p>\n", msg.content()))
    }

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
        // clear the cookie from `login`
        // .cookie(Cookie::build("_flash", "").max_age(Duration::ZERO).finish())
        .body(body)

    // let mut resp = HttpResponse::Ok()
    //     .content_type(ContentType::html())
    //     .body(body);
    // // `add_removal_cookie` is more explicit that we are removing the cookie
    // resp.add_removal_cookie(&Cookie::new("_flash", "")).unwrap();
    // resp
}
