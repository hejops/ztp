use std::fmt::Debug;
use std::fmt::Display;

use tokio::task::JoinError;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::delivery::init_delivery_worker;
use zero_to_prod::idempotency::init_expiry_worker;
use zero_to_prod::startup::Application;
use zero_to_prod::telemetry::get_subscriber;
use zero_to_prod::telemetry::init_subscriber;

fn report_exit(
    name: &str,
    // damn, how do you derive this type? beats me...
    outcome: Result<Result<(), impl Debug + Display>, JoinError>,
) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{name} exited gracefully")
        }

        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain=?e,
                error.message=%e,
                "{name} failed (inner)"
            )
        }

        Err(e) => {
            tracing::error!(
                error.cause_chain=?e,
                error.message=%e,
                "{name} failed (outer)"
            )
        }
    }
}

// note how async must be propagated everywhere;
// "unless polled, there is no guarantee that [futures] will execute to
// completion"

/// Initialise telemetry, load config, and start the server
#[tokio::main] // requires tokio features: macros, rt-multi-thread
async fn main() -> Result<(), anyhow::Error> {
    // RUST_LOG default is "error": https://docs.rs/env_logger/latest/env_logger/#enabling-logging
    // only logs at the specified level and higher are emitted

    // env_logger::Builder::from_env(Env::default().default_filter_or("info")).
    // init();

    let subscriber = get_subscriber("ztp", "info", std::io::stdout);
    init_subscriber(subscriber);

    // notes:
    // - `msg` (defined in span) is always printed in all-caps
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

    // let server = Application::build(cfg).await?;
    // server.run_until_stopped().await?;

    let server = Application::build(cfg.clone()).await?.run_until_stopped();
    let delivery_worker = init_delivery_worker(cfg.clone());
    let expiry_worker = init_expiry_worker(cfg);

    // If `spawn` is not called, all async branches are run on the same thread, and
    // the branches run concurrently, but -not- in parallel. If one branch
    // blocks the thread, -all- other branches will be unable to continue!

    let server_thread = tokio::spawn(server);
    let delivery_worker_thread = tokio::spawn(delivery_worker);
    let expiry_worker_thread = tokio::spawn(expiry_worker);

    // Waits on multiple concurrent branches, returning when the **first** branch
    // completes, cancelling the remaining branches.
    tokio::select! {
        // if let-ish syntax:
        // result = task => { do_stuff(result) }
        o = server_thread => { report_exit("API", o) },
        o = delivery_worker_thread => { report_exit("Background delivery worker", o) },
        o = expiry_worker_thread => { report_exit("Background expiry worker", o) },
    }

    // note: the last function call is wrapped by tokio (so LSP can't reach it)
    Ok(())
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
