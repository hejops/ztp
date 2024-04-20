pub mod configuration;
pub mod routes;
pub mod startup;

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
