use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::web;
use actix_web::web::Form;
use actix_web::App;
use actix_web::HttpResponse;
use actix_web::HttpServer;
use serde::Deserialize;

#[allow(dead_code)]
fn ch0() {
    // preface {{{
    // cloud-native applications must have high availability (distributed),
    // and handle dynamic workloads (elastic)

    // TDD and CI will be important

    // type system should make undesirable states difficult or impossible to
    // represent

    // code must be expressive enough to solve the problem, but flexible enough
    // to be allowed to evolve. run first, optimise later}}}
}

#[allow(dead_code)]
fn ch1() {
    // installation, tooling, CI {{{

    // inner development loop: write, compile, run, test

    // faster linking with lld:
    //
    // # - Arch, `sudo pacman -S lld clang`
    // Cargo.toml
    // [target.x86_64-unknown-linux-gnu]
    // rustflags = ["-C", "linker=clang", "-C", "link-arg=-fuse-ld=lld"]

    // project watcher:
    // cargo install cargo-watch
    // cargo watch -x check -x test -x run

    // code coverage:
    // cargo install cargo-tarpaulin
    // cargo tarpaulin --ignore-tests

    // linting:
    // rustup component add clippy
    // cargo clippy
    // cargo clippy -- -D warnings

    // formatting:
    // rustup component add rustfmt
    // cargo fmt
    // cargo fmt -- --check

    // security:
    // cargo install cargo-audit
    // cargo audit}}}
}

/// The server is not responsible for binding to an address, it only listens to
/// an already bound address.
///
/// API endpoints:
/// `/health_check` (GET)
pub fn ch2_3(
    // address: &str, // fixed port
    listener: TcpListener,
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

    /// Note: viewing http response requires `curl -v`
    // async fn health_check() -> impl Responder { HttpResponse::Ok() }
    async fn health_check() -> HttpResponse { HttpResponse::Ok().finish() }

    #[derive(Deserialize)]
    struct FormData {
        name: String,
        email: String,
    }

    /// If invalid data is encountered, the function returns early, and is
    /// transformed into an `Error` (400) automatically, even if `form` is not
    /// used.
    ///
    /// Note: if the function takes no arguments, it will always return 200,
    /// even on invalid data.
    ///
    /// # Request parsing
    ///
    /// How `Form` -> `Result` extraction works: `FromRequest` trait provides
    /// the `from_request` method, which takes `HttpRequest` + `Payload`,
    /// and implicitly 'wraps' the return value as `Result<Self, Self::Error>`
    /// (in practice, this usually means (200, 400)).
    ///
    /// Under the hood, `from_request` uses `UrlEncoded::new`, and
    /// `serde_urlencoded::from_bytes`.
    ///
    /// # Deserialization, serde
    ///
    /// `serde` defines a set of data models, agnostic to specific data formats
    /// like JSON.
    ///
    /// The `Serialize` trait (`serialize` method) converts a single type `T`
    /// (e.g. `Vec`) into `Result`:
    ///
    /// ```rust,ignore
    ///     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    /// ```
    ///
    /// The `Serializer` trait (`serialize_X` methods) converts any and all
    /// arbitrary Rust types `T` into `Result`.
    ///
    /// Monomorphisation is a zero-cost abstraction (no runtime cost). Proc
    /// macros (`#[derive(Deserialize)]`) make parsing convenient.
    async fn subscribe(form: Form<FormData>) -> HttpResponse { HttpResponse::Ok().finish() }

    // `HttpServer` handles transport level concerns, such as TCP sockets,
    // concurrent connections, TLS, etc
    //
    // an `App` 'lives' in a `HttpServer`, and handles all request/response logic
    // via `route` endpoints

    let server = HttpServer::new(|| {
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

        // .route("/", web::get().to(greet))
        // web::get() is syntactic sugar for:
        // .route("/", actix_web::Route::new().guard(actix_web::guard::Get()))
        // .route("/{name}", web::get().to(greet))
        // `name` is just an arg; the captured value is passed to the handler
        // function, where it should be extracted
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
