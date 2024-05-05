// use super::subscriber_name::SubscriberName;

use super::SubscriberEmail;
use super::SubscriberName;

pub struct NewSubscriber {
    pub name: SubscriberName,
    // pub email: String,
    pub email: SubscriberEmail,
}
