use std::net::TcpListener;

use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::web;
use actix_web::web::Data;
use actix_web::App;
use actix_web::HttpServer;
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use actix_web_lab::middleware::from_fn;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

use crate::authentication::reject_anonymous_users;
use crate::configuration::DatabaseSettings;
use crate::configuration::Settings;
use crate::email_client::EmailClient;
use crate::routes::admin_dashboard;
use crate::routes::change_password;
use crate::routes::change_password_form;
use crate::routes::confirm;
use crate::routes::health_check;
use crate::routes::home;
use crate::routes::login;
use crate::routes::login_form;
use crate::routes::logout;
use crate::routes::newsletter_form;
use crate::routes::publish_newsletter;
use crate::routes::subscribe;

/// Wrapper for actix's `Server` with access to the bound port. Not to be
/// confused with actix's `App`!
pub struct Application {
    /// Left private; use `get_port` to access
    port: u16,
    /// Contains the following components: TCP listener (randomised port), db
    /// pool (fixed port), and email client
    server: Server,
}

impl Application {
    /// Wrapper over `startup::run` that builds a `Server`
    pub async fn build(cfg: Settings) -> Result<Self, anyhow::Error> {
        // // hardcoded host (localhost), fixed port (8000)
        // let addr = format!("127.0.0.1:{}", cfg.application.port);

        // env-dependent host
        let addr = format!("{}:{}", cfg.application.host, cfg.application.port);
        let listener = TcpListener::bind(addr)?;

        // get the randomised port assigned by OS; this will be saved in the `port`
        // field
        let port = listener.local_addr().unwrap().port();

        // for distributed database, PostgreSQL should generally be used as a first
        // option as it is "easy to run locally and in CI via Docker,
        // well-supported within the Rust ecosystem".
        //
        // sqlx is chosen for compile-time safety and async support; `diesel` is unique
        // in having a DSL that makes queries reusable within `diesel`, but not
        // outside
        //
        // cargo install sqlx-cli --no-default-features --features rustls,postgres
        //
        // querying a (postgres) db can be done via psql (CLI) or `sqlx::PgConnection`
        // (Rust)

        // connect_lazy only connects when the pool is used for the first time (this is
        // not async). this allows db-free requests (e.g. health_check) to avoid
        // init'ing the db. however, attempting to init the db when it is -not- yet
        // configured (e.g. in docker) will cause HTTP 500 to be returned

        // let pool = PgPool::
        //     // connect(cfg.database.connection_string().expose_secret()).await
        // //     connect_lazy(cfg.database.connection().expose_secret()) // &str
        // // .expect("postgres must be running; run scripts/init_db.sh");
        // connect_lazy_with(cfg.database.connection()); // PgConnectOptions

        // in the book, `PgPool` is changed to `PgPoolOptions` during refactor without
        // really explaining why
        // let pool = PgPoolOptions::new().connect_lazy_with(cfg.database.connection());
        let pool = get_connection_pool(&cfg.database);

        let sender = cfg.email_client.sender().unwrap();
        let timeout = cfg.email_client.timeout();
        let email_client = EmailClient::new(
            cfg.email_client.base_url,
            sender,
            cfg.email_client.authorization_token,
            timeout,
        );

        let server = run(
            listener,
            pool,
            email_client,
            cfg.application.base_url,
            cfg.application.hmac_secret,
            cfg.redis_uri,
        )
        .await?;

        Ok(Self { port, server })
    }

    pub fn get_port(&self) -> u16 { self.port }

    /// Because this consumes `self`, this should be the final function call (or
    /// passed to `tokio::spawn`)
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> { self.server.await }
}

pub fn get_connection_pool(db_cfg: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(db_cfg.connection())
}

/// Wrapper for top-level application `base_url` (because raw `String`s may
/// conflict with one another when passed around by `Data`)
pub struct AppBaseUrl(pub String);

/// Message authentication guarantees that the message has not been modified in
/// transit, and allows identity of the sender to be verified. We use HMAC
/// (specified in RFC2104).
#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

/// The server is not responsible for binding to an address, it only listens to
/// an already bound address.
// Requires a running Redis instance (?).
///
/// Declares all API endpoints.
pub async fn run(
    // address: &str, // fixed port
    listener: TcpListener,
    pool: PgPool,
    email_client: EmailClient,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
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

    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());

    // client side cookies
    let cookie_store = CookieMessageStore::builder(secret_key.clone()).build();
    let msg_framework = FlashMessagesFramework::builder(cookie_store).build();

    // server side sessions
    // required only for persistent logins; all other parts of the app can work
    // without redis
    let redis_store = RedisSessionStore::new(redis_uri.expose_secret()).await?;

    // `Data` is externally an `Arc` (for sharing/cloning), internally a `HashMap`
    // (for wrapping arbitrary types)
    let pool = web::Data::new(pool);
    let email_client = web::Data::new(email_client);

    // note the closure; "`actix-web` will spin up a worker process for each
    // available core on your machine. Each worker runs its own copy of the
    // application built by `HttpServer` calling the very same closure that
    // `HttpServer::new` takes as argument. That is why `connection` has to be
    // cloneable - we need to have one for every copy of `App`."
    let server = HttpServer::new(move || {
        // order is probably not significant, but the book declares wrappers, then
        // routes, then app data
        App::new()
            // .wrap(Logger::default())
            .wrap(TracingLogger::default()) // wrap the whole app in tracing middleware
            .wrap(msg_framework.clone()) // like tracing, but for the browser
            // .wrap(session_store.clone())
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            // essentially equivalent to a `match` block, where we try to exhaust a series
            // of routes (match arms). this process is common to all API frameworks.
            // remember, the guard must match the client's request type
            .route("/", web::get().to(home))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            // .route("/newsletters", web::post().to(publish))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/dashboard", web::get().to(admin_dashboard))
                    .route("/password", web::get().to(change_password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(logout))
                    .route("/newsletters", web::get().to(newsletter_form))
                    .route("/newsletters", web::post().to(publish_newsletter)),
            )
            // with `.app_data`, global state (e.g. db connection, http client(s)) is made available
            // to all endpoints, if specified as args. args passed must either implement
            // `Clone` or be wrapped with `web::Data`. the latter is preferred as -all-
            // associated fields of the struct can be shared across the app.
            .app_data(pool.clone())
            .app_data(email_client.clone())
            // .app_data(base_url.clone())
            .app_data(Data::new(AppBaseUrl(base_url.clone())))
            .app_data(Data::new(HmacSecret(hmac_secret.clone())))

        // .route("/", web::get().to(greet))
        //
        // web::get() is syntactic sugar for:
        // .route("/", actix_web::Route::new().guard(actix_web::guard::Get()))
        //
        // `name` is just an arg; the captured value is passed to the handler
        // function at runtime, where it should be extracted
        // (try changing `name` to `foo` both here and in `greet`)
        //
        // .route("/{name}", web::get().to(greet))
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

    // tests were moved from src/main.rs to src/lib.rs, keeping only the
    // entrypoint in src/main.rs. later, tests were again moved from src/lib.rs
    // to a dedicated tests dir; see tests/main.rs for details
}
