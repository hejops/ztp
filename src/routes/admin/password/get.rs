use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::IncomingFlashMessages;
use actix_web_flash_messages::Level;

/// `GET /admin/password`
pub async fn change_password_form(
    // session: TypedSession,
    // user_id: web::ReqData<UserId>,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    // if session.get_user_id().map_err(error_500)?.is_none() {
    //     return Ok(redirect("/login"));
    // };

    // let user_id = user_id.into_inner();

    // copied from `login_form`
    let mut error_msg = String::new();
    for msg in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        error_msg.push_str(&format!("<p><i>{}</i></p>\n", msg.content()))
    }

    let body = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Change Password</title>
</head>
<body>
    {error_msg}
    <form action="/admin/password" method="post">
        <label>Current password
            <input
                type="password"
                placeholder="Enter current password"
                name="current_password"
            >
        </label>
        <br>
        <label>New password
            <input
                type="password"
                placeholder="Enter new password"
                name="new_password"
            >
        </label>
        <br>
        <label>Confirm new password
            <input
                type="password"
                placeholder="Type the new password again"
                name="new_password_check"
            >
        </label>
        <br>
        <button type="submit">Change password</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#
    );

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(body))
}
