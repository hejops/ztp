-- remove salt, since we store PHCs and let argon2 take care of salting
ALTER TABLE users DROP COLUMN salt;

