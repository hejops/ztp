use actix_web::http::header::ContentType;
use actix_web::web;
use actix_web::HttpResponse;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct QueryParams {
    error: Option<String>,
}

/// `GET` endpoint (`login_form`)
pub async fn login_form(query: web::Query<QueryParams>) -> HttpResponse {
    // message authentication guarantees that the message has not been modified in
    // transit and allows you to verify the identity of the sender. we use HMAC
    // (specified in RFC2104)

    let error_msg = match query.0.error {
        None => "".to_owned(),
        Some(e) => format!(
            "<p><i>{}</i></p>",
            // e,
            htmlescape::encode_minimal(&e)
        ),
    };

    // let body = include_str!("./login.html"); // static html

    // warning: injecting query params like this easily opens the door to XSS!
    // https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html
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
    <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/form#action -->
    <form action="/login" method="post">
      <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/label -->
      <label>
        Username
        <!-- https://developer.mozilla.org/en-US/docs/Web/HTML/Element/Input -->
        <!-- http://localhost:8000/login?username=foo&password=bar -->
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
