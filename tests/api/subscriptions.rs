use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::Mock;
use wiremock::ResponseTemplate;

use crate::helpers::spawn_app;

/// Test the `/subscriptions` endpoint with valid request
#[tokio::test]
async fn subscribe_ok() {
    let app = spawn_app().await;
    let body = "name=john&email=foo%40bar.com";

    // simulate sending an email; this is required because
    // `subscriptions::subscribe` sends an email, but our test method
    // `post_subscriptions` doesn't
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let resp = app.post_subscriptions(body.to_owned()).await;

    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.status().is_success());
    // assert_eq!(resp.content_length().unwrap(), 0); // empty body
}

/// Test that the new user is added to (and can be retrieved from) db
#[tokio::test]
async fn subscribe_added_to_db() {
    let app = spawn_app().await;
    let body = "name=john&email=foo%40bar.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.to_owned()).await;

    // now we check that the side-effect occurred (subscription added to db). in the
    // absence of a separate (GET) endpoint ('client-side'), the check can be
    // done inside the test ('server-side')

    let added = sqlx::query!("SELECT email, name, status FROM subscriptions")
        .fetch_one(&app.pool)
        .await
        .unwrap();

    // initially, this failed because we didn't actually do anything (i.e. INSERT)
    // with the `subscribe` request, so we couldn't fetch anything
    assert_eq!(added.name, "john");
    assert_eq!(added.email, "foo@bar.com");
    // remember to add `status` to the `SELECT` statement!
    assert_eq!(added.status, "pending_confirmation");

    // since email is a UNIQUE field, this test can only pass once (per db
    // instantation)! to avoid this, either restart db (not good), implement
    // rollbacks (faster), or create a new db with every test (easier -- handled
    // by spawn_app)

    // PGPASSWORD=password psql --host=localhost --username=postgres
    // --command='SELECT datname FROM pg_catalog.pg_database;'
    // datname --------------------------------------
    //  postgres
    //  newsletter
    //  template1
    //  template0
    //  9ebef8e3-598f-4467-93ff-9e687625d063
    //  f4671add-aec8-4c67-8953-a79d979f4274
    //  4da2016f-1b27-4256-bdd6-d2f77ddadafa
    //  ...
    // (13 rows)
}

/// Test the `/subscriptions` endpoint with invalid requests (missing/invalid
/// fields)
#[tokio::test]
async fn subscribe_invalid_request() {
    let app = spawn_app().await;

    // for parametrised testing, use `rstest`
    for (body, msg) in [
        ("", "null"),
        ("name=john", "null email"),
        ("email=foo%40bar.com", "null name"),
        // confusingly, the book first tests that invalid inputs return 200, only changing it 400
        // later
        // https://github.com/LukeMathWalker/zero-to-production/commit/6db241eef
        ("name=&email=foo%40bar.com", "empty name"),
        ("name=john&email=", "empty email"),
        ("name=john&email=not-an-email", "invalid email"),
    ] {
        let resp = app.post_subscriptions(body.to_owned()).await;
        assert_eq!(resp.status().as_u16(), 400, "{msg}");
    }
}

/// Test the `/subscriptions` endpoint with valid request, and verify that the
/// confirmation email contains a valid confirmation url
#[tokio::test]
async fn subscribe_ok_with_confirmation() {
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

    assert_eq!(links.text, links.html)
}
