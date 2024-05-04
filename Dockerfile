# to (re)build:
# sudo docker build --tag ztp --file Dockerfile .
#
# note: when hosted, docker pull is used instead to skip the slow build

# note: to build this image successfully, sqlx offline mode is required, as
# .env is not read in docker. i.e. generate a .sqlx directory with:
# cargo sqlx prepare --workspace

# to run:
# sudo docker run ztp

# to clean:
# sudo docker ps --all
# sudo docker container rm <container-id>
# sudo docker rmi ztp:latest
#
# (if build failed, or after rebuild)
# sudo docker system prune
# https://docs.docker.com/config/pruning/#prune-everything

# docker volumes, images, etc are stored in /var/lib/docker. while it is
# possible to configure docker to store data in an external drive, this should
# really only be done when remaining disk space (incl images) falls below 5 GB
#
# https://docs.docker.com/config/daemon/#daemon-data-directory
# https://wiki.archlinux.org/title/Docker#Configuration

# to reduce image size, ignore target in .dockerignore, discard compilation
# artifacts, and use bookworm-slim runtime image:
#
# rust 1.77.2 base: 1.41 GB
# default build: 5.94
# dockerignore target/: 3.16
# discard build runtime: 1.43
# rust-slim: 0.77
# bookworm-slim: 0.1

# to reduce compilation time, cache expensive but slowly changing operations
# first, i.e. building dependencies.
#
# default: 1:40
# chef, 1st: 1:45
# chef, cached: 15 s
# chef, cached, no change: < 0.1 s
#
# note that this creates a bunch of dangling images (which are all caches that
# don't actually use up any disk space). consequently, this speed up is lost if
# docker is always pruned
#
# https://docs.docker.com/build/cache/

# https://hub.docker.com/_/rust/
# FROM rust:1.72.0

# 1. init (this image will be discarded)

# # sqlx 0.7 is incompatible with rust 1.72.0, apparently
# # https://github.com/LukeMathWalker/zero-to-production/issues/259
# FROM rust:1.77.2-slim AS builder

# this is a full-sized image, but we use it for cached building
FROM lukemathwalker/cargo-chef:latest-rust-1.77.2 as chef

# WORKDIR = mkdir + cd
# https://docs.docker.com/reference/dockerfile/#workdir
WORKDIR /app

# Install the required system dependencies for our linking configuration
RUN apt update && apt install lld clang -y
# if using the slim image, some dependencies must be installed
# RUN apt update && apt install pkg-config libssl-dev lld clang -y

# 2. calculate and build deps, reusing the previous layer
FROM chef as planner

# "paths of files and directories [of <src>] will be interpreted as **relative
# to the source of the context of the build**"
# i.e. first . is the local machine, second . is the image
# https://docs.docker.com/reference/dockerfile/#copy
# https://stackoverflow.com/a/55034801

COPY . .
RUN cargo chef prepare --recipe-path recipe.json
FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Up to this point, if our dependency tree stays the same,
# all layers should be cached.

# 3. build
COPY . .
# use the prepared .sqlx directory for compilation
ENV SQLX_OFFLINE true
RUN cargo build --release
# RUN cargo build --release --bin zero2prod

# 4. release
# FROM rust:1.77.2-slim AS runtime
FROM debian:bookworm-slim AS runtime

# OpenSSL is dynamically linked by some of our dependencies
# ca-certificates is needed to verify TLS certificates when establishing HTTPS connections
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/zero-to-prod zero-to-prod

# We need the configuration file at runtime!
COPY configuration configuration
ENV APP_ENVIRONMENT production
# ENV RUST_BACKTRACE full
# the binary name corresponds to the `name` declared in Cargo.toml
ENTRYPOINT ["./zero-to-prod"]

# sudo docker run ztp
# curl http://127.0.0.1:8000/health_check -v
#
# curl: (7) Failed to connect to 127.0.0.1 port 8000 after 0 ms: Couldn't connect to server
#
# docker did not establish port 8000

# sudo docker run -p 8000:8000 ztp
# curl http://127.0.0.1:8000/health_check -v
#
# *   Trying 127.0.0.1:8000...
# * Connected to 127.0.0.1 (127.0.0.1) port 8000
# > GET /health_check HTTP/1.1
# > Host: 127.0.0.1:8000
# curl: (56) Recv failure: Connection reset by peer
#
# docker established port 8000, but the request (from host) was not registered,
# since it did not originate from localhost
