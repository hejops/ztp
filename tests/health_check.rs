use std::net::TcpListener;

use sqlx::Connection;
use sqlx::PgConnection;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::startup;

// 'no external crate' -- add to Cargo.toml:
// [lib]
// path = "src/lib.rs"

// integration tests remove the need for manual curl invocation
//
// black-box tests are most robust, as they reflect exactly how clients
// interact with API (e.g. request type, path)
//
// testing should be framework-agnostic, and common between testing and
// production

// must not be async! https://github.com/LukeMathWalker/zero-to-production/issues/242#issuecomment-1915933810
/// Generally, `Server`s should be `spawn`ed. Requests from a `Client` should be
/// made `async`.
///
/// Returns the address to which the server was bound, in the form `http://127.0.0.1:{port}`.
/// The `http://` prefix is important, as clients will send requests to the address.
fn spawn_app() -> String {
    // port 0 is reserved by the OS; the server will be spawned on a random
    // available port. this port must then be made known to clients
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = startup::run(listener).expect("bind address");
    tokio::spawn(server);
    format!("http://127.0.0.1:{port}")
}

// "when a tokio runtime is shut down all tasks spawned on it are dropped.
// tokio::test spins up a new runtime at the beginning of each test case and
// they shut down at the end of each test case."

#[tokio::test]
async fn health_check() {
    let addr = spawn_app(); // spawn the server in background (not async)
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{addr}/health_check"))
        // send, await, handle error
        .send()
        .await
        .expect("execute request");
    assert!(resp.status().is_success());
    assert_eq!(resp.content_length().unwrap(), 0); // empty body
}

#[tokio::test]
async fn subscribe_ok() {
    let addr = spawn_app();
    let client = reqwest::Client::new();

    let cfg = get_configuration().unwrap();
    PgConnection::connect(&cfg.database.connection_string())
        .await
        .expect("Is postgres running? Run scripts/init_db.sh");

    let body = "name=john&email=foo%40bar.com";
    let resp = client
        .post(format!("{addr}/subscriptions"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("execute request");
    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.status().is_success());
    // assert_eq!(resp.content_length().unwrap(), 0); // empty body

    // now we also need to check that the side-effect occurred (subscription
    // added to db). ideally, this should be done with another separate (GET)
    // endpoint, but if this endpoint is non-trivial to implement, the check can
    // be done inside the test ('server-side')
}

#[tokio::test]
async fn subscribe_invalid() {
    let addr = spawn_app();
    let client = reqwest::Client::new();

    // for parametrised testing, use rstest
    for (body, msg) in [
        ("name=john", "no email"),
        ("email=foo%40bar.com", "no name"),
        ("", "empty"),
    ] {
        let resp = client
            .post(format!("{addr}/subscriptions"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("execute request");
        assert_eq!(resp.status().as_u16(), 400, "{msg}");
    }
}
