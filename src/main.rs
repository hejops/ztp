use std::net::TcpListener;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::startup::run;
use zero_to_prod::telemetry::get_subscriber;
use zero_to_prod::telemetry::init_subscriber;

// note how async must be propagated everywhere;
// "unless polled, there is no guarantee that [futures] will execute to
// completion"

#[tokio::main] // requires tokio features: macros, rt-multi-thread
async fn main() -> Result<(), std::io::Error> {
    // RUST_LOG default is "error": https://docs.rs/env_logger/latest/env_logger/#enabling-logging
    // only logs at the specified level and higher are emitted

    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).
    // init();

    let subscriber = get_subscriber("ztp", "info", std::io::stdout);
    init_subscriber(subscriber);

    // notes:
    // - `msg` (defined in span) is always all-caps
    // - key order may not be same across events (but this doesn't matter)
    // - only END events have elapsed_milliseconds key
    // {"v":0,"name":"ztp","msg":"[ADDING NEW SUBSCRIBER - EVENT] Added new subscriber to db","level":30,"hostname":"hostname","pid":53179,"time":"2024-04-23T07:48:23.364866355Z","target":"zero_to_prod::routes::subscriptions","line":119,"file":"src/routes/subscriptions.rs","id":"5214805c-3998-407a-b4e4-bdd796a81be6","subscriber_name":"John","subscriber_email":"john@foo.com"}

    // 127.0.0.1 is a 'magic' address that refers to localhost, i.e. "this machine".
    //
    // https://serverfault.com/a/502721
    // https://www.rfc-editor.org/rfc/rfc5735#section-3
    //
    // when server is bound to this address, the server only accepts (listens to)
    // requests originating from the same machine. thus, requests can be made as
    // host:curl -> host:server (dev), but not host:curl -> container:server.
    //
    // thus, the prod server should be made to accept requests originating from any
    // address

    // let addr = "127.0.0.1:0"; // randomised port
    let cfg = get_configuration().unwrap();

    // // hardcoded localhost:8000
    // let addr = format!("127.0.0.1:{}", cfg.application.port);

    let addr = format!("{}:{}", cfg.application.host, cfg.application.port);
    let listener = TcpListener::bind(addr)?;

    // taken from subscribe_ok
    let cfg = get_configuration().unwrap();
    let pool = PgPool::
        // connect(cfg.database.connection_string().expose_secret()).await
        // only connect when the pool is used for the first time (this is not async). this allows
        // db-free requests (e.g. health_check) to avoid init'ing the db. however, attempting to
        // init the db when it is not yet configured (e.g. in docker) will cause HTTP
        // 500 to be returned
        connect_lazy(cfg.database.connection_string().expose_secret())
    .expect("postgres must be running; run scripts/init_db.sh");

    // note: our `run` function is now wrapped by tokio (so LSP can't reach it)
    run(listener, pool)?.await
}

// /// when expanded with `cargo expand`
// fn main() -> Result<(), std::io::Error> {
//     let body = async { ch2_3().await };
//     {
//         return tokio::runtime::Builder::new_multi_thread()
//             .enable_all()
//             .build()
//             .expect("Failed building the Runtime")
//             .block_on(body);
//     }
// }
