use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::web;
use actix_web::App;
use actix_web::HttpServer;
use sqlx::PgPool;

use crate::routes::health_check;
use crate::routes::subscribe;

/// The server is not responsible for binding to an address, it only listens to
/// an already bound address.
///
/// API endpoints:
/// `/health_check` (GET)
pub fn run(
    // address: &str, // fixed port
    listener: TcpListener,
    pool: PgPool,
) -> Result<Server, std::io::Error> {
    // email newsletter (e.g. MailChimp)

    // before implementing any features, consider potential "user stories" that
    // describe who, what, and why
    //
    // always develop a MVP first, then iterate to improve fault tolerance, add
    // features, etc

    // web framework: actix-web (tokio)

    // generally, handler functions should have this type signature:
    // async fn foo(req: HttpRequest) -> impl Responder {}

    // async fn greet(req: HttpRequest) -> impl Responder {
    //     let name = req.match_info().get("name").unwrap_or("world");
    //     format!("Hello {}", name)
    // }

    // `HttpServer` handles transport level concerns, such as TCP sockets,
    // concurrent connections, TLS, etc
    //
    // an `App` 'lives' in a `HttpServer`, and handles all request/response logic
    // via `route` endpoints

    // `Data` is externally an `Arc` (for sharing/cloning), internally a `HashMap`
    // (for wrapping arbitrary types)
    let pool = web::Data::new(pool);

    // note the closure; "`actix-web` will spin up a worker process for each
    // available core on your machine. Each worker runs its own copy of the
    // application built by `HttpServer` calling the very same closure that
    // `HttpServer::new` takes as argument. That is why `connection` has to be
    // cloneable - we need to have one for every copy of `App`."
    let server = HttpServer::new(move || {
        // essentially equivalent to a `match` block, where we try to exhaust a series
        // of routes (match arms)

        // endpoint: GET /health_check

        // endpoint: POST /subscriptions
        // who: visitors
        // what: subscribe to blog
        // why: receive email updates
        // e.g. /name=john&email=foo%40bar.com (application/x-www-form-urlencoded)

        App::new()
            .route("/health_check", web::get().to(health_check))
            // remember, the guard must match the client's request type
            .route("/subscriptions", web::post().to(subscribe))
            // global state
            .app_data(pool.clone())

        // .route("/", web::get().to(greet))
        // web::get() is syntactic sugar for:
        // .route("/", actix_web::Route::new().guard(actix_web::guard::Get()))
        // .route("/{name}", web::get().to(greet))
        // `name` is just an arg; the captured value is passed to the handler
        // function at runtime, where it should be extracted
        // (try changing `name` to `foo` both here and in `greet`)
        //
        // https://actix.rs/docs/url-dispatch/#resource-pattern-syntax
    })
    // .bind(address)? // if no port specified, "invalid socket address"
    .listen(listener)?
    .run();

    // server.await // async return -- caller uses foo().await

    Ok(server) // sync return -- caller uses foo()?.await

    // ~/ > curl 127.0.0.1:5748/ajsdkl
    // Hello ajsdkl
    //
    // ~/ > curl 127.0.0.1:5748
    // Hello world

    // where to place tests:
    // 1. embedded (with #[cfg(test)]): good for unit testing, easy access to
    //    private objects, never exposed to users
    // 2. /tests dir: for integration testing
    // 3. doctests: (not discussed)

    // to allow testing, we move almost everything to src/lib.rs, keeping only
    // the entrypoint in src/main.rs

    // for distributed database, PostgreSQL should generally be used as a first
    // option as it is "easy to run locally and in CI via Docker, well-supported
    // within the Rust ecosystem".
    //
    // sqlx is chosen for compile-time safety and async support; `diesel` is
    // unique in having a DSL that makes queries reusable within `diesel`, but
    // not outside
    //
    // cargo install sqlx-cli --no-default-features --features rustls,postgres
    //
    // querying a (postgres) db can be done via psql (CLI) or
    // `sqlx::PgConnection` (Rust)
}
