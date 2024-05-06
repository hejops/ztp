use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check() {
    let app = spawn_app().await; // spawn the server in background (not async)
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health_check", app.addr))
        // send, await, handle error
        .send()
        .await
        .expect("execute request");
    assert!(resp.status().is_success());

    // note that the last statement is wrapped by `tokio`
    assert_eq!(resp.content_length().unwrap(), 0); // empty body
}
