#!/usr/bin/env bash
set -euo pipefail

# pass a non-null value for SKIP_DOCKER to update db schema without
# (re)starting docker

# https://wiki.archlinux.org/title/Docker
# https://docs.docker.com/config/daemon/start/
# https://hub.docker.com/_/postgres

# sudo systemctl start docker
# sudo docker info
# sudo docker pull postgres
# sudo docker ps
# sudo docker ps --format json | jq -r '. | select(.Image="postgres") | .ID' | xargs sudo docker stop

# sudo docker pull dpage/pgadmin4

# sudo docker run -p 80:80 -e PGADMIN_DEFAULT_EMAIL=user@domain.com -e PGADMIN_DEFAULT_PASSWORD=SuperSecret -d dpage/pgadmin4

cd "$(realpath "$0" | xargs dirname | xargs dirname)"

if [ ! -x "$(command -v psql)" ]; then
	echo >&2 "Error: psql is not installed."
	exit 1
elif [ ! -x "$(command -v sqlx)" ]; then
	echo >&2 "Error: sqlx is not installed."
	echo >&2 "Use:"
	echo >&2 "    cargo install --version='~0.7' sqlx-cli \
--no-default-features --features rustls,postgres"
	echo >&2 "to install it."
	exit 1
fi

# Check if a custom user has been set, otherwise default to 'postgres'
DB_USER="${POSTGRES_USER:=postgres}"
# Check if a custom password has been set, otherwise default to 'password'
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
# Check if a custom database name has been set, otherwise default to 'newsletter'
DB_NAME="${POSTGRES_DB:=newsletter}"
# Check if a custom port has been set, otherwise default to '5432'
DB_PORT="${POSTGRES_PORT:=5432}"
# Check if a custom host has been set, otherwise default to 'localhost'
DB_HOST="${POSTGRES_HOST:=localhost}"

if [[ -z ${SKIP_DOCKER:=} ]]; then
	# Launch postgres using Docker
	# this is idempotent; if port is already bound, it cannot be reused
	sudo docker run \
		-e POSTGRES_USER=${DB_USER} \
		-e POSTGRES_PASSWORD=${DB_PASSWORD} \
		-e POSTGRES_DB=${DB_NAME} \
		-p "${DB_PORT}":5432 \
		-d postgres \
		postgres -N 1000
# ^ Increased maximum number of connections for testing purposes
fi

# wait for Postgres to be healthy before starting to run commands against it
export PGPASSWORD="${DB_PASSWORD}"
until psql --host="${DB_HOST}" --username="${DB_USER}" --port="${DB_PORT}" --dbname="postgres" --command='\q'; do
	>&2 echo "Postgres is still unavailable - sleeping"
	sleep 1
done
>&2 echo "Postgres is up and running on port ${DB_PORT}!"

DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_NAME}
export DATABASE_URL

# these are idempotent when postgres already running

set -x

sqlx database create

# export DATABASE_URL=postgres://postgres:password@127.0.0.1:5432/newsletter
# sqlx migrate add create_subscriptions_table # creates migrations/xxx.sql

# requires 'migrations' directory
sqlx migrate run

# pgadmin4 -- no good PKGBUILD
# phppgadmin -- where's the binary? https://aur.archlinux.org/cgit/aur.git/tree/PKGBUILD?h=phppgadmin

PGPASSWORD=password psql --host=localhost --username=postgres --command='\c newsletter' --command='\dt'

#               List of relations
#  Schema |       Name       | Type  |  Owner
# --------+------------------+-------+----------
#  public | _sqlx_migrations | table | postgres
#  public | subscriptions    | table | postgres
# (2 rows)
