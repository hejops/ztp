use crate::helpers::check_redirect;
use crate::helpers::spawn_app;

#[tokio::test]
async fn no_cookies() {
    let app = spawn_app().await;
    let login_body = serde_json::json!({
        "username": "username",
        "password": "password",
    });
    let resp = app.post_login(&login_body).await;
    assert_eq!(resp.status().as_u16(), 303);
    check_redirect(&resp, "/login").await;

    // let cookies: HashSet<_> =
    // resp.headers().get_all("Set-Cookie").into_iter().collect(); println!("{:?
    // }", cookies); assert!(cookies.contains(&
    // reqwest::header::HeaderValue::from_str("_foo").unwrap()));

    // cookie setting/removal is handled as flash messages; we don't have a way to
    // test it

    // let cookie = resp.cookies().find(|c| c.name() == "_flash").unwrap();
    // // println!("{:?}", cookie);
    // assert_eq!(cookie.value(), "You are not authorized to view this page.");

    let html = app.get_login_html().await;
    // println!("{}", html);
    assert!(html.contains("<p><i>You are not authorized to view this page.</i></p>"));

    // error should not persist on reload
    let html = app.get_login_html().await;
    assert!(!html.contains("<p><i>You are not authorized to view this page.</i></p>"));
}
