use std::net::TcpListener;

use sqlx::PgPool;
use zero_to_prod::configuration::get_configuration;
use zero_to_prod::startup::run;

// note how async must be propagated everywhere
// "unless polled, there is no guarantee that [futures] will execute to
// completion"

#[tokio::main] // requires features macros, rt-multi-thread
async fn main() -> Result<(), std::io::Error> {
    // let addr = "127.0.0.1:0"; // hardcoded port
    let cfg = get_configuration().unwrap();
    let addr = format!("127.0.0.1:{}", cfg.application_port);
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
