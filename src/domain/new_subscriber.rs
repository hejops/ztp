use super::SubscriberEmail;
use super::SubscriberName;

pub struct NewSubscriber {
    pub name: SubscriberName,
    // pub email: String,
    pub email: SubscriberEmail,
    // sub_id: Uuid
    // status: enum (pending/active)
}
