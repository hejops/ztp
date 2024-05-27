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

    let wrong = "wrong_password";
    let body = serde_json::json!({
        "current_password": "foo",
        "new_password": wrong,
        "new_password_repeat": wrong,
    });

    let app = spawn_app().await;

    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/login");
}

#[tokio::test]
async fn new_passwords_do_not_match() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    app.post_login(&body).await;

    let body = serde_json::json!({
        "current_password": app.test_user.password,
        "new_password": "foo",
        "new_password_repeat": "bar",
    });
    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/admin/password");

    assert!(app
        .get_change_password_html()
        .await
        .contains("The two passwords supplied do not match!"));
}

#[tokio::test]
async fn current_password_incorrect() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    app.post_login(&body).await;

    let new_pw = "wrong_password";
    let body = serde_json::json!({
        "current_password": "foo",
        "new_password": new_pw,
        "new_password_repeat": new_pw,
    });
    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/admin/password");

    assert!(app
        .get_change_password_html()
        .await
        .contains("The current password is incorrect!"));
}

#[tokio::test]
async fn new_password_too_short() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    app.post_login(&body).await;

    let new_pw = "abc";
    let body = serde_json::json!({
        "current_password": app.test_user.password,
        "new_password": new_pw,
        "new_password_repeat": new_pw,
    });
    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/admin/password");

    assert!(app
        .get_change_password_html()
        .await
        .contains("The new password is too short!"));
}

#[tokio::test]
async fn logout() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&body).await;
    check_redirect(&resp, "/admin/dashboard");

    assert!(app
        .get_admin_dashboard_html()
        .await
        .contains(&format!("Welcome {}", app.test_user.username)));

    let resp = app.post_logout().await;
    check_redirect(&resp, "/login");

    assert!(app
        .get_login_html()
        .await
        .contains("You have successfully logged out."));

    let resp = app.get_admin_dashboard().await;
    check_redirect(&resp, "/login");
}

/// Login, change password, logout, then login again!
#[tokio::test]
async fn change_password_ok() {
    let app = spawn_app().await;

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": app.test_user.password,
    });
    let resp = app.post_login(&body).await;
    check_redirect(&resp, "/admin/dashboard");

    assert!(app
        .get_admin_dashboard_html()
        .await
        .contains(&format!("Welcome {}", app.test_user.username)));

    let new_pw = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "current_password": app.test_user.password,
        "new_password": new_pw,
        "new_password_repeat": new_pw,
    });
    let resp = app.post_change_password(&body).await;
    check_redirect(&resp, "/admin/password");

    assert!(app
        .get_change_password_html()
        .await
        .contains("Password changed successfully."));

    let resp = app.post_logout().await;
    check_redirect(&resp, "/login");

    assert!(app
        .get_login_html()
        .await
        .contains("You have successfully logged out."));

    let resp = app.get_admin_dashboard().await;
    check_redirect(&resp, "/login");

    let body = serde_json::json!({
        "username": app.test_user.username,
        "password": new_pw,
    });
    let resp = app.post_login(&body).await;
    check_redirect(&resp, "/admin/dashboard");
}
