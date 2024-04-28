# to (re)build:
# sudo docker build --tag ztp --file Dockerfile .
#
# note: when hosted, docker pull is used instead to skip the slow build

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
#
# to reduce image size, ignore target in .dockerignore, discard compilation
# artifacts, and use slim image:
#
# rust 1.77.2 base: 1.41 GB
# default build: 5.94
# dockerignore target/: 3.16
# discard build runtime: 1.43
# slim: 0.77

# note: to build this image successfully, sqlx offline mode is required, as
# .env is not read in docker. i.e. generate a .sqlx directory with:
# cargo sqlx prepare --workspace

# https://hub.docker.com/_/rust/
# FROM rust:1.72.0

# build (this image will be discarded)

# sqlx 0.7 is incompatible with rust 1.72.0, apparently
# https://github.com/LukeMathWalker/zero-to-production/issues/259
FROM rust:1.77.2-slim AS builder

# WORKDIR = mkdir + cd
# https://docs.docker.com/reference/dockerfile/#workdir
WORKDIR /app

# Install the required system dependencies for our linking configuration
# note: the slim image lacks some dependencies
# RUN apt update && apt install lld clang -y
RUN apt update && apt install pkg-config libssl-dev lld clang -y

# "paths of files and directories [of <src>] will be interpreted as **relative
# to the source of the context of the build**"
# i.e. first . is the local machine, second . is the image
# i.e. `WORKDIR /app; COPY . .` is equivalent to `COPY . /app`
# https://docs.docker.com/reference/dockerfile/#copy
# https://stackoverflow.com/a/55034801
COPY . .

# use the prepared .sqlx directory for compilation
ENV SQLX_OFFLINE true
RUN cargo build --release

# Launch the binary; the name corresponds to the `name` declared in Cargo.toml
ENV APP_ENVIRONMENT production
ENTRYPOINT ["./target/release/zero-to-prod"]

# release

FROM rust:1.77.2-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/zero-to-prod zero-to-prod
# We need the configuration file at runtime!
COPY configuration configuration
ENV APP_ENVIRONMENT production
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
