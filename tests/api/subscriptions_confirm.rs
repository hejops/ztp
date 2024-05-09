use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::Mock;
use wiremock::ResponseTemplate;

use crate::helpers::spawn_app;

/// Test `/subscriptions/confirm` with no confirmation token
#[tokio::test]
async fn confirmation_no_token() {
    let app = spawn_app().await;
    let resp = reqwest::get(format!("{}/subscriptions/confirm", app.addr))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

/// Test the `/subscriptions/confirm` endpoint with valid request, and verify
/// that the confirmation url returns 200 when requested
#[tokio::test]
async fn confirm_ok() {
    let app = spawn_app().await;
    let body = "name=john&email=foo%40bar.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.to_owned()).await;

    let email_reqs = app.email_server.received_requests().await.unwrap();

    let resp = reqwest::get(app.get_confirmation_links(&email_reqs[0]).text)
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

/// Test that requesting the confirmation url modifies the user's `status` in
/// the db
#[tokio::test]
async fn confirm_modifies_user_status_in_db() {
    let app = spawn_app().await;
    let body = "name=john&email=foo%40bar.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.to_owned()).await;

    let email_reqs = app.email_server.received_requests().await.unwrap();

    let links = app.get_confirmation_links(&email_reqs[0]);

    reqwest::get(links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // copied from `subscribe_added_to_db`
    let added = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(added.name, "john");
    assert_eq!(added.email, "foo@bar.com");
    assert_eq!(added.status, "confirmed");
}
