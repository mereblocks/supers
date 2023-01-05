use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Registry};

// Convenience function to initialize tracing.
// It sets a default directive to ignore logs with empty messages, and
// it reads from the environment variable RUST_LOG, as usual.
pub fn init_tracing() {
    LogTracer::init().expect("cannot init logger");
    let filter = EnvFilter::builder()
        .with_regex(true)
        .with_default_directive("supers=debug".parse().unwrap())
        .from_env()
        .expect("error parsing RUST_LOG environment variable");
    let subs = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_thread_names(true)
        .with_line_number(true)
        .with_file(true)
        .json()
        .with_span_list(false)
        .flatten_event(true)
        .with_writer(std::io::stderr)
        .finish();
    // let formatting_layer =
    //     BunyanFormattingLayer::new("supers".into(), std::io::stderr)
    //         .with_filter(filter);
    // let subscriber = Registry::default()
    //     .with(JsonStorageLayer)
    //     .with(formatting_layer);
    let _ = tracing::subscriber::set_global_default(subs);
}
