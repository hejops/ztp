# Zero to Production

<!-- good review(s) of the book: -->
<!-- https://bitemyapp.com/blog/notes-on-zero2prod-rust/ -->

## Ch 0

- cloud-native applications must have high availability (distributed),
  and handle dynamic workloads (elastic)

- TDD and CI will be important

- type system should make undesirable states difficult or impossible to
  represent

- code must be expressive enough to solve the problem, but flexible enough
  to be allowed to evolve; run first, optimise later

## Ch 1

- installation, tooling, CI
- inner development loop: write, compile, run, test

1. faster linking with lld:

   ```toml
   # Arch: `sudo pacman -S lld clang`
   # Cargo.toml
   [target.x86_64-unknown-linux-gnu]
   rustflags = ["-C", "linker=clang", "-C", "link-arg=-fuse-ld=lld"]
   ```

1. project watcher:

   ```sh
   cargo install cargo-watch
   cargo watch -x test -x run
   ```

1. code coverage:

   ```sh
    cargo install cargo-tarpaulin
    cargo tarpaulin --ignore-tests
   ```

1. linting:

   ```sh
    rustup component add clippy
    cargo clippy
    cargo clippy -- -D warnings
   ```

1. formatting:

   ```sh
    rustup component add rustfmt
    cargo fmt
    cargo fmt -- --check
   ```

1. security:

   ```sh
    cargo install cargo-audit
    cargo audit
   ```

## Ch 5

- "Production environments [focus on] running our software to make it available
  to our users. Anything that is not strictly related to that goal is either a
  waste of resources, at best, or a security liability, at worst."

- Virtualisation (Docker) allows you to use a self-contained environment and
  get away with saying "it works on my machine" when you deploy. Hosting
  (DigitalOcean) lets you actually deploy the thing.
