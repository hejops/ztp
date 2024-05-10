-- We wrap the whole migration in a transaction to make sure it succeeds or
-- fails atomically (all or nothing). `sqlx` does not do it automatically for
-- us.
--
-- https://www.postgresql.org/docs/current/sql-begin.html
-- further reading: "Designing Data-Intensive Applications" (M. Kleppmann)
begin
;
-- old entries (before the schema update) will have the `status` field
-- null; fix this
    UPDATE subscriptions
        SET status = 'confirmed'
        WHERE status IS NULL;
-- Make `status` mandatory now
    ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
commit
;

