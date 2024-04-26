use std::net::TcpListener;

use sqlx::PgPool;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::BunyanFormattingLayer;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing_log::LogTracer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::startup::run;

// note how async must be propagated everywhere;
// "unless polled, there is no guarantee that [futures] will execute to
// completion"

#[tokio::main] // requires tokio features: macros, rt-multi-thread
async fn main() -> Result<(), std::io::Error> {
    // RUST_LOG default is "error": https://docs.rs/env_logger/latest/env_logger/#enabling-logging
    // only logs at the specified level and higher are emitted

    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).
    // init();

    // required for `actix_web` logs to be captured by `Subscriber`
    LogTracer::init().unwrap();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")); // requires feature `env-filter`
    let fmt_layer = BunyanFormattingLayer::new("ztp".into(), std::io::stdout);
    let subscriber = Registry::default()
        // does order matter?
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(fmt_layer);
    set_global_default(subscriber).unwrap();

    // notes:
    // - `msg` (defined in span) is always all-caps
    // - key order may not be same across events (but this doesn't matter)
    // - only END events have elapsed_milliseconds key
    // {"v":0,"name":"ztp","msg":"[ADDING NEW SUBSCRIBER - EVENT] Added new subscriber to db","level":30,"hostname":"hostname","pid":53179,"time":"2024-04-23T07:48:23.364866355Z","target":"zero_to_prod::routes::subscriptions","line":119,"file":"src/routes/subscriptions.rs","id":"5214805c-3998-407a-b4e4-bdd796a81be6","subscriber_name":"John","subscriber_email":"john@foo.com"}

    // let addr = "127.0.0.1:0"; // randomised port
    let cfg = get_configuration().unwrap();
    let addr = format!("127.0.0.1:{}", cfg.application_port); // hardcoded 8000
    let listener = TcpListener::bind(addr)?;

    // taken from subscribe_ok
    let cfg = get_configuration().unwrap();
    let conn = PgPool::connect(&cfg.database.connection_string())
        .await
        .expect("postgres must be running; run scripts/init_db.sh");

    run(listener, conn)?.await
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
