// fn main not required
mod health_check;
mod helpers;
mod subscriptions;
mod subscriptions_confirm;

// 'no external crate' -- add to Cargo.toml:
// [lib]
// path = "src/lib.rs"

// On testing, logging and tracing
//
// integration tests remove the need for manual curl invocation
//
// black-box tests are most robust, as they reflect exactly how clients interact
// with API (e.g. request type, path)
//
// testing should be framework-agnostic, and common between testing and
// production
//
// however, tests are not proofs of correctness, and there will be known
// unknowns (e.g. dropped connection, malicious inputs), and unknown unknowns
// (e.g. heavy load, multiple failures, memory leaks). crucially, the latter
// cannot be reproduced; to react to such issues, we need to generate
// high-quality logs, and be able to interpret them.
//
// the standard crate for logging is `log` (which provides -only- macros);
// `actix_web::middleware` also provides `Logger`. a separate crate is required
// for the `Log` trait, which makes the global decision of what to do with all
// the logs (e.g. print? write to file? send to remote?); we use `env_logger`
//
// good logs must be verbose and reproducible; the goal is to be able to find
// the cause of a bug with logs alone, and as little user clarification as
// possible. where possible, all user inputs and timestamps must be recorded.
//
// logging is done at the level of individual instructions; only a flat series
// of logs can ever be produced, and trying to stitch them together into a
// tree-like structure quickly leads to scaling issues
//
// tracing is done at the higher level of tasks, and allows the granular
// division of tasks (into subtasks, etc) to be represented with ease. for this,
// `Subscriber` is analogous to `Log`, provided by the `tracing-subscriber`
// crate.

// note: `tracing` events can be picked up `Log`, with the `log` feature.
// however, logging is still useful at the top level, to capture
// framework-related logs that don't need spans. since `log` events cannot be
// picked up by `Subscriber`, we use `tracing-log` to do this. tl;dr:
//
// `log` -> `Log`
// `tracing` -> `Subscriber`
// `tracing` -[tracing-log]> `Log`
// `log` -[-F log]> `Subscriber`

// "when a tokio runtime is shut down all tasks spawned on it are dropped.
// tokio::test spins up a new runtime at the beginning of each test case and
// they shut down at the end of each test case."

// why test? risk mitigation, documentation, modularity

// integration tests are built in target/debug/deps (one per tests/*.rs file or
// tests/* directory; usually with multiple builds)
//
// some options exist, from simplest to most ideal:
//
// tests/some_test.rs
//
// tests/helpers.rs -- helpers is -not- an integration test!
// tests/helpers/mod.rs -- submodule, but when `use`d by a test, compiler will
// complain about unused functions
//
// tests/api/main.rs
// tests/api/helpers.rs
// tests/api/some_test.rs
// (can be flat, no further subdirs required, unless a test gets too large)

// an added benefit of grouping tests in a single dir: "While each executable is
// compiled in parallel, the linking phase is instead entirely sequential!
// Bundling all your test cases in a single executable reduces the time spent
// compiling your test suite in CI."
