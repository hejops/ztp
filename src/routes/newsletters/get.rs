use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use uuid::Uuid;

/// `GET /admin/newsletters`
pub async fn newsletter_form(
    flash_messages: IncomingFlashMessages
) -> Result<HttpResponse, actix_web::Error> {
    let mut error_msg = String::new();
    for msg in flash_messages.iter() {
        //.filter(|m| m.level() == Level::Error) {
        // the book calls `writeln!(String)`, which is no longer allowed?
        // writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
        error_msg.push_str(&format!("<p><i>{}</i></p>\n", msg.content()))
    }

    // generated per request
    let key = Uuid::new_v4().to_string();

    // the book uses 2 input boxes for content (text/html), but i don't feel like
    // doing this

    let body = format!(
        r#"
<!doctype html>
<html lang="en">
  <head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8" />
    <title>Submit new issue</title>
  </head>
  {error_msg}
  <body>
    <form action="/admin/newsletters" method="post">
      <label>
        Title
        <input type="text" placeholder="Enter Title" name="title" />
      </label>

      <label>
        Content
        <input type="text" placeholder="Enter Content" name="content" />
      </label>

      <!-- damn, people actually do this? -->
      <input hidden type="text" name="idempotency_key" value="{key}">

      <button type="submit">Submit</button>
    </form>
  </body>
</html>
    "#
    );

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
