use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::BunyanFormattingLayer;
use tracing_bunyan_formatter::JsonStorageLayer;
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Registry;

/// 'subscriber' is a `tracing` trait, and is not to be confused with a
/// subscriber of the newsletter!
/// Note: `sink` must be a closure (e.g. `std::io::stdout`), not a return value.
pub fn get_subscriber<Sink>(
    name: &str,
    filter_level: &str,
    sink: Sink,
) -> impl Subscriber
where
    // higher-ranked trait bound; sink must `implement` the `MakeWriter` trait for all choices of the
    // lifetime parameter `'a`
    Sink: for<'a> MakeWriter<'a> + 'static, // + Send + Sync,
{
    // requires feature `env-filter`
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter_level));
    let fmt_layer = BunyanFormattingLayer::new(
        name.to_string(),
        // std::io::stdout
        sink,
    );
    Registry::default()
        // does order matter?
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(fmt_layer)
}

/// Start the logger and subscriber. This should be called before starting the
/// db/app.
///
/// The trait bounds of `subscriber` are derived from the type signature of
/// `set_global_default`
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().unwrap(); // required for `actix_web` logs to be captured by `Subscriber`
    set_global_default(subscriber).unwrap();
}
