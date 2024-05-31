-- schema:
--
-- - user id + idempotency key (composite primary)
-- - timestamp
-- - http response
-- --- status code (smallint)
-- --- headers ([(text, bytea)]) -- multiple headers can have same name, so no hashmap!
-- --- body (bytea)
--
-- note: because more than one level of nesting is not supported, response is flat
-- (not a struct)
-- https://github.com/launchbadge/sqlx/issues/1031
--
-- composite type
-- https://www.postgresql.org/docs/current/sql-createtype.html
-- "Postgres creates an array type implicitly when we run a CREATE TYPE
-- statement - it is simply the composite type name prefixed by an
-- underscore."
CREATE TYPE header_pair AS (
    name TEXT,
    value BYTEA
);
CREATE TABLE idempotency (
   user_id uuid NOT NULL REFERENCES users(user_id),
   idempotency_key TEXT NOT NULL,

   created_at timestamptz NOT NULL,

   response_status_code SMALLINT NOT NULL,
   response_headers header_pair[] NOT NULL,
   response_body BYTEA NOT NULL,

   PRIMARY KEY(user_id, idempotency_key)
);

