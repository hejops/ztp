use std::net::TcpListener;

use once_cell::sync::Lazy;
use sqlx::Connection;
use sqlx::Executor;
use sqlx::PgConnection;
use sqlx::PgPool;
use uuid::Uuid;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::configuration::DatabaseSettings;
use zero_to_prod::startup;
use zero_to_prod::telemetry::get_subscriber;
use zero_to_prod::telemetry::init_subscriber;

// 'no external crate' -- add to Cargo.toml:
// [lib]
// path = "src/lib.rs"

// On testing, logging and tracing
//
// integration tests remove the need for manual curl invocation
//
// black-box tests are most robust, as they reflect exactly how clients interact
// with API (e.g. request type, path)
//
// testing should be framework-agnostic, and common between testing and
// production
//
// however, tests are not proofs of correctness, and there will be known
// unknowns (e.g. dropped connection, malicious inputs), and unknown unknowns
// (e.g. heavy load, multiple failures, memory leaks). crucially, the latter
// cannot be reproduced; to react to such issues, we need to generate
// high-quality logs, and be able to interpret them.
//
// the standard crate for logging is `log` (which provides -only- macros);
// `actix_web::middleware` also provides `Logger`. a separate crate is required
// for the `Log` trait, which makes the global decision of what to do with all
// the logs (e.g. print? write to file? send to remote?); we use `env_logger`
//
// good logs must be verbose and reproducible; the goal is to be able to find
// the cause of a bug with logs alone, and as little user clarification as
// possible. where possible, all user inputs and timestamps must be recorded.
//
// logging is done at the level of individual instructions; only a flat series
// of logs can ever be produced, and trying to stitch them together into a
// tree-like structure quickly leads to scaling issues
//
// tracing is done at the higher level of tasks, and allows the granular
// division of tasks (into subtasks, etc) to be represented with ease. for this,
// `Subscriber` is analogous to `Log`, provided by the `tracing-subscriber`
// crate.

// note: `tracing` events can be picked up `Log`, with the `log` feature.
// however, logging is still useful at the top level, to capture
// framework-related logs that don't need spans. since `log` events cannot be
// picked up by `Subscriber`, we use `tracing-log` to do this. tl;dr:
//
// `log` -> `Log`
// `tracing` -> `Subscriber`
// `tracing` -[tracing-log]> `Log`
// `log` -[-F log]> `Subscriber`

pub struct TestApp {
    pub addr: String,
    pub pool: PgPool,
}

/// Read `DatabaseSettings` and create a db with a randomised name (but with the
/// same migrations/tables, specified in the migrations directory). The
/// connection to this db can then be used to run a single test.
pub async fn configure_database(cfg: &DatabaseSettings) -> PgPool {
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
    sqlx::migrate!().run(&pool).await.unwrap();
    pool
}

/// Init a static subscriber using the `once_cell` crate; alternatives include
/// `std::cell:OnceCell` and `lazy_static` crate.
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
    // different "closure types" to the same var is not allowed by the compiler.

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

// must not be async! https://github.com/LukeMathWalker/zero-to-production/issues/242#issuecomment-1915933810
/// Spawn a `TestApp` containing default config, which can be used for testing;
/// a wrapper over `startup::run`.
//
// Generally, `Server`s should be `spawn`ed. Requests from a `Client` should be
// made `async`.
///
/// Returns the address to which the server was bound, in the form `http://127.0.0.1:{port}`, as
/// well as the address to the (randomised) postgres connection.
/// The `http://` prefix is important, as this is the address that clients will send requests to.
async fn spawn_app() -> TestApp {
    // init the subscriber once only
    Lazy::force(&TRACING);

    // port 0 is reserved by the OS; the server will be spawned on an address with a
    // random available port. this address/port must then be made known to clients
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let addr = format!("http://127.0.0.1:{port}");

    // in addition to the address, the db connection must also be made known. db
    // name is randomised to allow a new db to be spawned per test
    let mut cfg = get_configuration().unwrap();
    // static db name
    // let pool = PgPool::connect(&cfg.database.connection_string())
    //     .await
    //     .expect("postgres must be running; run scripts/init_db.sh");
    // random db name
    cfg.database.database_name = Uuid::new_v4().to_string();
    let pool = configure_database(&cfg.database).await;

    let server = startup::run(listener, pool.clone()).expect("bind address");
    tokio::spawn(server);

    TestApp { addr, pool }
}

// "when a tokio runtime is shut down all tasks spawned on it are dropped.
// tokio::test spins up a new runtime at the beginning of each test case and
// they shut down at the end of each test case."

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

#[tokio::test]
async fn subscribe_ok() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=john&email=foo%40bar.com";
    let resp = client
        .post(format!("{}/subscriptions", app.addr))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("execute request");
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

#[tokio::test]
async fn subscribe_invalid() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    // for parametrised testing, use `rstest`
    for (body, msg) in [
        ("name=john", "no email"),
        ("email=foo%40bar.com", "no name"),
        ("", "empty"),
    ] {
        let resp = client
            .post(format!("{}/subscriptions", app.addr))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("execute request");
        assert_eq!(resp.status().as_u16(), 400, "{msg}");
    }
}
