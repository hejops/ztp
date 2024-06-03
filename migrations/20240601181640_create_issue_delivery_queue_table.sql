CREATE TABLE issue_delivery_queue(
   newsletter_issue_id uuid NOT NULL
      REFERENCES newsletter_issues (newsletter_issue_id),
   subscriber_email TEXT NOT NULL,

   -- TODO: add a n_retries and execute_after columns to keep track of how many
   -- attempts have already taken place and how long we should wait before
   -- trying again.
   PRIMARY KEY (newsletter_issue_id, subscriber_email)
);
