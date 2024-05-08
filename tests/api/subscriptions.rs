use crate::helpers::spawn_app;

/// Test the `/subscriptions` endpoint with valid request
#[tokio::test]
async fn subscribe_ok() {
    let app = spawn_app().await;
    let body = "name=john&email=foo%40bar.com";
    let resp = app.post_subscriptions(body.to_owned()).await;

    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.status().is_success());
    // assert_eq!(resp.content_length().unwrap(), 0); // empty body

    // now we check that the side-effect occurred (subscription added to db).
    // ideally, this should be done with another separate (GET) endpoint, but if
    // this endpoint is non-trivial to implement, the check can be done inside
    // the test ('server-side')

    let added = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    // initially, this failed because we didn't actually do anything (i.e. INSERT)
    // with the subscribe request, so we couldn't fetch anything
    assert_eq!(added.name, "john");
    assert_eq!(added.email, "foo@bar.com");

    // since email is a UNIQUE field, this test can only pass once (per db
    // instantation)! to avoid this, either restart db (not good), implement
    // rollbacks (faster), or create a new db with every test (easier)

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

// by default, calling assert!(foo.is_ok()) does not reveal the `Err` in cargo
// test:
//
// ---- dummy_fail stdout ----
// thread 'dummy_fail' panicked at tests/health_check.rs:236:5:
// assertion failed: result.is_ok()
//
// with claims::assert_ok(result):
//
// ---- dummy_fail stdout ----
// thread 'dummy_fail' panicked at tests/health_check.rs:244:5:
// assertion failed, expected Ok(..), got Err("The app crashed due to an IO
// error")

/// Test the `/subscriptions` endpoint with invalid requests (missing/invalid
/// fields)
#[tokio::test]
async fn subscribe_invalid() {
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
