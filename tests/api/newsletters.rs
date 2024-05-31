use std::time::Duration;

use wiremock::matchers::any;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::Mock;
use wiremock::ResponseTemplate;

use crate::helpers::check_redirect;
use crate::helpers::spawn_app;
use crate::helpers::ConfirmationLinks;
use crate::helpers::TestApp;

// we are no longer concerned with validating the structure of Newsletter
// because we now expect data to be provided via html form

// #[tokio::test]
// async fn invalid_newsletter() {
//     let app = spawn_app().await;
//     app.login(&app.test_user.username, &app.test_user.password)
//         .await;
//
//     for (body, msg) in [
//         (
//             serde_json::json!({
//                     "content": {
//                         "text": "bar",
//                         "html": "<p>baz</p>",
//                     }
//             }),
//             "no title",
//         ),
//         (serde_json::json!({ "title": "foo" }), "no content"),
//     ] {
//         let resp = app.post_newsletters(&body).await;
//         assert_eq!(resp.status().as_u16(), 400, "{msg}");
//     }
// }

#[tokio::test]
async fn not_logged_in() {
    let app = spawn_app().await;
    let resp = app.get_newsletters().await;
    assert_eq!(resp.status().as_u16(), 303);
    check_redirect(&resp, "/login");

    app.login("no-user", "foo").await;
    let resp = app.get_newsletters().await;
    assert_eq!(resp.status().as_u16(), 303);
    check_redirect(&resp, "/login");

    app.login(&app.test_user.username, "foo").await;
    let resp = app.get_newsletters().await;
    assert_eq!(resp.status().as_u16(), 303);
    check_redirect(&resp, "/login");

    // we don't test POST because if GET requires creds, then so will POST
}

/// Add a subscriber to a (typically empty) db, but don't confirm
async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=john&email=foo%40bar.com";

    // (scoped) mocks must always be assigned and -named-!
    let _mock = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        // with `mount_as_scoped`, this `Mock` remains local, and will not interfere with the
        // caller's ("global") `Mock`. local assertions are also performed at the end of this
        // function (eagerly)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    // see `subscribe_ok_with_confirmation`
    let email_reqs = app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(&email_reqs)
}

/// Simulate `/subscriptions/confirm`
async fn create_confirmed_subscriber(app: &TestApp) {
    let link = create_unconfirmed_subscriber(app).await;
    reqwest::get(link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

#[tokio::test]
async fn no_confirmed_subscribers() {
    let app = spawn_app().await;
    app.login(&app.test_user.username, &app.test_user.password)
        .await;

    create_unconfirmed_subscriber(&app).await;

    let _ = Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let contents = serde_json::json!({
        "title": "foo",
        // "content": {
        //     "text": "bar",
        //     "html": "<p>baz</p>",
        // }
        "content": "bar",
        "idempotency_key": "baz",
    });

    let resp = app.post_newsletters(&contents).await;
    check_redirect(&resp, "/admin/newsletters");

    assert!(app
        .get_newsletters_html()
        .await
        .contains("New issue published successfully."));
}

#[tokio::test]
async fn one_confirmed_subscriber() {
    let app = spawn_app().await;
    app.login(&app.test_user.username, &app.test_user.password)
        .await;

    create_confirmed_subscriber(&app).await;

    let _ = Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let contents = serde_json::json!({
        "title": "foo",
        // "content": {
        //     "text": "bar",
        //     "html": "<p>baz</p>",
        // }
        "content": "bar",
        "idempotency_key": "baz",
    });

    let resp = app.post_newsletters(&contents).await;
    check_redirect(&resp, "/admin/newsletters");

    assert!(app
        .get_newsletters_html()
        .await
        .contains("New issue published successfully."));
}

#[tokio::test]
async fn idempotent() {
    let app = spawn_app().await;
    app.login(&app.test_user.username, &app.test_user.password)
        .await;

    create_confirmed_subscriber(&app).await;

    // 2 requests, but only 1 email sent/received
    let _ = Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let contents = serde_json::json!({
        "title": "foo",
        "content": "bar",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let resp = app.post_newsletters(&contents).await;
    check_redirect(&resp, "/admin/newsletters");

    assert!(app
        .get_newsletters_html()
        .await
        .contains("New issue published successfully."));

    let resp = app.post_newsletters(&contents).await;
    check_redirect(&resp, "/admin/newsletters");

    assert!(app
        .get_newsletters_html()
        .await
        .contains("Issue has already been published."));
}

#[tokio::test]
async fn concurrent() {
    let app = spawn_app().await;
    app.login(&app.test_user.username, &app.test_user.password)
        .await;

    create_confirmed_subscriber(&app).await;

    let _ = Mock::given(path("/email"))
        .and(method("POST"))
        // long delay ensures that the second request arrives before the first one completes
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let contents = serde_json::json!({
        "title": "foo",
        "content": "bar",
        // both requests will have the same idempotency_key, violating uniqueness constraint
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let resp1 = app.post_newsletters(&contents); // don't await!
    let resp2 = app.post_newsletters(&contents);
    let (resp1, resp2) = tokio::join!(resp1, resp2);
    assert_eq!(resp1.status(), resp2.status());
    assert_eq!(resp1.text().await.unwrap(), resp2.text().await.unwrap());
}
