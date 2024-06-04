-- how many attempts have already taken place
-- TODO: NOT NULL
ALTER TABLE issue_delivery_queue ADD COLUMN n_retries SMALLINT DEFAULT 0;
-- how long we should wait before trying again; should double with every retry
ALTER TABLE issue_delivery_queue ADD COLUMN execute_after SMALLINT DEFAULT 2;
