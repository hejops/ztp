use std::net::TcpListener;

use zero_to_prod::ch2_3;

// note how async must be propagated everywhere
// "unless polled, there is no guarantee that [futures] will execute to
// completion"

#[tokio::main] // requires features macros, rt-multi-thread
async fn main() -> Result<(), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    ch2_3(listener)?.await
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
