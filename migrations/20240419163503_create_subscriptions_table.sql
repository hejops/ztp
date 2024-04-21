-- Add migration script here
CREATE TABLE subscriptions(
   -- all fields required
   -- https://www.postgresql.org/docs/current/datatype-uuid.html
   id uuid NOT NULL, -- synthetic identifier
   PRIMARY KEY (id),

   -- UNIQUE adds B-tree index
   -- https://www.postgresql.org/docs/current/indexes-unique.html
   email TEXT NOT NULL UNIQUE, -- natural identifier

   -- https://www.postgresql.org/docs/current/datatype-datetime.html
   name TEXT NOT NULL,

   subscribed_at timestamptz NOT NULL
);

