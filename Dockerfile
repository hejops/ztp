# to (re)build:
# sudo docker build --tag ztp --file Dockerfile .

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

# REPOSITORY       TAG       IMAGE ID       CREATED          SIZE
# ztp              latest    96f8dfc4bc01   20 seconds ago   5.94GB
# rust             1.77.2    14bb4f02fb0e   2 weeks ago      1.41GB

# note: to build this image successfully, at least 6 GB disk space is required
# (2.8 for build context), as well as sqlx offline mode (i.e. a .sqlx
# directory; .env is not read in docker).

# cargo sqlx prepare --workspace

# https://hub.docker.com/_/rust/
# FROM rust:1.72.0

# sqlx 0.7 is incompatible with rust 1.72.0, apparently
# https://github.com/LukeMathWalker/zero-to-production/issues/259
FROM rust:1.77.2

# WORKDIR = mkdir + cd
# https://docs.docker.com/reference/dockerfile/#workdir
WORKDIR /app

# Install the required system dependencies for our linking configuration
RUN apt update && apt install lld clang -y

# Copy all files from our working environment to our Docker image
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

# sudo docker run ztp
# curl http://127.0.0.1:8000/health_check -v
#
# *   Trying 127.0.0.1:8000...
# * connect to 127.0.0.1 port 8000 from 127.0.0.1 port 33650 failed: Connection refused
# * Failed to connect to 127.0.0.1 port 8000 after 0 ms: Couldn't connect to server
# * Closing connection
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
# > User-Agent: curl/8.7.1
# > Accept: */*
# >
# * Request completely sent off
# * Recv failure: Connection reset by peer
# * Closing connection
# curl: (56) Recv failure: Connection reset by peer
#
# docker established port 8000, but the request (from host) was not registered,
# since it did not originate from localhost
