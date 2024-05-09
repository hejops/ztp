use linkify::Link;
use linkify::LinkFinder;
use linkify::LinkKind;
use once_cell::sync::Lazy;
use reqwest::Url;
use serde_json::Value;
use sqlx::Connection;
use sqlx::Executor;
use sqlx::PgConnection;
use sqlx::PgPool;
use uuid::Uuid;
use wiremock::MockServer;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::configuration::DatabaseSettings;
use zero_to_prod::startup::get_connection_pool;
use zero_to_prod::startup::Application;
use zero_to_prod::telemetry::get_subscriber;
use zero_to_prod::telemetry::init_subscriber;

/// Init a static subscriber using the `once_cell` crate; alternatives include
/// `std::cell:OnceCell` and `lazy_static` (crate).
// https://docs.rs/once_cell/latest/once_cell/#faq
// https://users.rust-lang.org/t/lazy-static-vs-once-cell-oncecell/58578
///
/// To opt in to verbose logging, use the env var `TEST_LOG`:
///
/// ```sh
///      TEST_LOG=true cargo test [test_name] | bunyan
/// ```
static TRACING: Lazy<()> = Lazy::new(|| {
    // `sink` is passed to `BunyanFormattingLayer::new`, which only requires `impl
    // MakeWriter`. however, the intuitive/'elegant' solution of assigning 2
    // different "closure types" to the same var is not allowed by the compiler,
    // hence the unwieldy match arms.

    // let sink = match std::env::var("TEST_LOG") {
    //     Ok(_) => std::io::stdout,
    //     Err(_) => std::io::sink,
    // };
    // let subscriber = get_subscriber("test", "debug", sink);
    // init_subscriber(subscriber);

    match std::env::var("TEST_LOG") {
        Ok(_) => {
            let subscriber = get_subscriber("test", "debug", std::io::stdout);
            init_subscriber(subscriber);
        }
        Err(_) => {
            let subscriber = get_subscriber("test", "debug", std::io::sink);
            init_subscriber(subscriber);
        }
    };
});

pub struct TestApp {
    pub addr: String,
    pub port: u16,
    pub pool: PgPool,
    pub email_server: MockServer,
}

pub struct ConfirmationLinks {
    pub text: Url,
    pub html: Url,
}

impl TestApp {
    /// Convenience method for making a `/subscriptions` `POST` request. While
    /// meant to mimic a `subscriptions::subscribe` a call, this method does
    /// -not- send email (necessary for successful result), so tests that use
    /// this method should simulate that separately (e.g. with `Mock`)
    pub async fn post_subscriptions(
        &self,
        body: String,
    ) -> reqwest::Response {
        let client = reqwest::Client::new();

        client
            .post(format!("{}/subscriptions", self.addr))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("execute request")
    }

    pub fn get_confirmation_links(
        &self,
        email_req: &wiremock::Request,
    ) -> ConfirmationLinks {
        // fn get_first_link(body: &str) -> Url {
        let get_first_link = |body: &str| {
            // closure is used to more easily capture self.port (fn would require extra arg)
            let links: Vec<Link> = LinkFinder::new()
                .links(body)
                .filter(|l| *l.kind() == LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let link = links[0].as_str().to_owned();

            let mut link = Url::parse(&link).unwrap();
            assert_eq!(link.host_str().unwrap(), "127.0.0.1");

            // retrieve the randomised port (assigned by OS)
            link.set_port(Some(self.port)).unwrap();
            link
        };

        let body: Value = serde_json::from_slice(&email_req.body).unwrap();

        // this will be `base_url`/subscriptions/confirm?subscription_token=...
        let text = get_first_link(body["TextBody"].as_str().unwrap());
        let html = get_first_link(body["HtmlBody"].as_str().unwrap());

        ConfirmationLinks { text, html }
    }
}

/// Read `DatabaseSettings` and create a db with a randomised name (but with the
/// same migrations/tables, specified in the `migrations` directory). The
/// connection to this db can then be used to run a single test.
async fn configure_database(cfg: &DatabaseSettings) -> PgPool {
    // connect to the top-level db
    let mut conn = PgConnection::connect_with(&cfg.connection_without_db())
        .await
        .expect("postgres must be running; run scripts/init_db.sh");

    // create randomised db (randomisation is done by caller, not here).
    // unlike `query!`, `Executor` trait must be imported, and query validity is not
    // checked at compile time
    conn.execute(format!(r#"CREATE DATABASE "{}";"#, cfg.database_name).as_str())
        .await
        .unwrap();

    // perform the migration(s) and create the table(s). `migrate!` path defaults to
    // "./migrations", where . is project root
    let pool = PgPool::connect_with(cfg.connection()).await.unwrap();
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to migrate");
    pool
}

// must not be async! https://github.com/LukeMathWalker/zero-to-production/issues/242#issuecomment-1915933810
/// Spawn a `TestApp` containing default config, which can be used for testing;
/// part of the setup is handled by `startup::run`.
//
// Generally, `Server`s should be `spawn`ed. Requests from a `Client` should be
// made `async`.
///
/// Returns the address to which the server was bound, in the form `http://127.0.0.1:{port}`, as
/// well as the address to the (randomised) postgres connection.
/// The `http://` prefix is important, as this is the address that clients will send requests to.
pub async fn spawn_app() -> TestApp {
    // init the tracing subscriber once only
    Lazy::force(&TRACING);

    // simulate mailchimp api
    let email_server = MockServer::start().await;

    let cfg = {
        // in addition to the address, the db connection must also be made known. db
        // name is randomised to allow a new db to be spawned per test
        let mut rand_cfg = get_configuration().unwrap();

        // // static db name
        // let pool = PgPool::connect(&cfg.database.connection_string())
        //     .await
        //     .expect("postgres must be running; run scripts/init_db.sh");

        // random db name
        rand_cfg.database.database_name = Uuid::new_v4().to_string();

        // port 0 is reserved by the OS; the server will be spawned on an address with a
        // random available port. this address/port must then be made known to clients
        rand_cfg.application.port = 0;

        rand_cfg.email_client.base_url = email_server.uri();

        rand_cfg
    };

    // most of the init is now done in `build`, but we now we need to retrieve the
    // randomised db port

    // we don't use this `pool` for TestApp, probably because the `pool` we really
    // need should be obtained -after- the server has been `spawn`ed
    let _pool = configure_database(&cfg.database).await;

    // let server = startup::run(listener, pool.clone(), email_client).unwrap();
    // let server = build(cfg.clone()).await.unwrap();
    let app = Application::build(cfg.clone()).await.unwrap();

    // previously, random port was retrieved here, and addr was declared
    // accordingly. however, since this is now abstracted away, we are left only
    // with a Server, which does not expose the random port. this must now be
    // retrieved via Application.get_port()

    // let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    // let port = listener.local_addr().unwrap().port();
    let addr = format!(
        // "http://127.0.0.1:{}",
        "http://localhost:{}",
        app.get_port()
    );
    let port = app.get_port(); // for constructing confirmation urls

    let pool = get_connection_pool(&cfg.database); // can be done before or after spawn, apparently
    tokio::spawn(app.run_until_stopped());

    TestApp {
        addr,
        port,
        pool,
        email_server,
    }
}