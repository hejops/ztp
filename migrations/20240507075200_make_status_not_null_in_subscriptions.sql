-- Add migration script here
-- We wrap the whole migration in a transaction to make sure
-- it succeeds or fails atomically. We will discuss SQL transactions
-- in more details towards the end of this chapter!
-- `sqlx` does not do it automatically for us.
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

