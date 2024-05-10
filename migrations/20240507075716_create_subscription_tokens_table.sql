-- important: this key is linked to the `subscriptions` table
--
-- "For each row in subscription_tokens there must exist a row in subscriptions
-- whose id field has the same value of subscriber_id, otherwise the insertion
-- fails."
CREATE TABLE subscription_tokens(
   subscriber_id uuid NOT NULL
      REFERENCES subscriptions (id),
   subscription_token TEXT NOT NULL,
   PRIMARY KEY (subscription_token)
);

