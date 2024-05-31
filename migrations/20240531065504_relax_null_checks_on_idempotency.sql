-- Our current implementation inserts [the saved response] after processing the
-- request, just before returning the response to the caller. We are going to
-- change that: we will insert a new row as soon as the handler is invoked. We
-- don’t know the final response at that point - we haven’t started processing
-- yet! We must relax the NOT NULL constraints on some of the columns:
--
-- dropping NOT NULL means the fields become Option<>
ALTER TABLE idempotency ALTER COLUMN response_status_code DROP NOT NULL;
ALTER TABLE idempotency ALTER COLUMN response_body DROP NOT NULL;
ALTER TABLE idempotency ALTER COLUMN response_headers DROP NOT NULL;

