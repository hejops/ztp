-- add salting to our crypto
ALTER TABLE users ADD COLUMN salt TEXT NOT NULL;

