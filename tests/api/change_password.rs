use uuid::Uuid;

use crate::helpers::check_redirect;
use crate::helpers::spawn_app;

/// Trying to `GET` or `POST` `/admin/password` when not logged in should
/// redirect to `/login`
#[tokio::test]
async fn not_logged_in() {
    let app = spawn_app().await;

    let resp = app.get_change_password().await;
    check_redirect(&resp, "/login");

    let app = spawn_app().await;

    let curr_pw = Uuid::new_v4().to_string();
    let new_pw = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "current_password": curr_pw,
        "new_password": new_pw,
        "new_password_repeat": new_pw,
    });

    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/login");
}
