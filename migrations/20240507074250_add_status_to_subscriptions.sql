-- Add migration script here
-- NULL == can be null, i.e. optional
ALTER TABLE subscriptions ADD COLUMN status TEXT NULL;

