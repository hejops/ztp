use linkify::Link;
use linkify::LinkFinder;
use linkify::LinkKind;
use reqwest::Url;
use serde_json::Value;
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
/// that the confirmation returns 200
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

    fn get_first_link(body: &str) -> String {
        let links: Vec<Link> = LinkFinder::new()
            .links(body)
            .filter(|l| *l.kind() == LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);
        links[0].as_str().to_owned()
    }

    let email_reqs = app.email_server.received_requests().await.unwrap();
    let body: Value = serde_json::from_slice(&email_reqs[0].body).unwrap();

    // this will be `base_url`/subscriptions/confirm?subscription_token=...
    let link = get_first_link(body["TextBody"].as_str().unwrap());

    let mut link = Url::parse(&link).unwrap();
    assert_eq!(link.host_str().unwrap(), "127.0.0.1");

    // retrieve the randomised port (assigned by OS)
    link.set_port(Some(app.port)).unwrap();

    let resp = reqwest::get(link).await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}
