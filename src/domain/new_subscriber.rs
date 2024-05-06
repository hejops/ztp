use super::SubscriberEmail;
use super::SubscriberName;

pub struct NewSubscriber {
    pub name: SubscriberName,
    // pub email: String,
    pub email: SubscriberEmail,
    // sub_id: Uuid
    // status: enum (pending/active)
}

// SMTP and REST can be used to send email; REST is usually easier to set up,
// but requires some provider -- we use Mailchimp since i don't have an email i
// can use with Postmark.
//
// https://github.com/LukeMathWalker/zero-to-production/issues/176#issuecomment-1490392528
//
// to actually talk with the REST API, we use a client (`reqwest`)
