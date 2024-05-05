mod new_subscriber;
mod subscriber_email;
mod subscriber_name;
// allow external `use` statements to skip `new_subscriber` etc
pub use new_subscriber::NewSubscriber;
pub use subscriber_email::SubscriberEmail;
pub use subscriber_name::SubscriberName;
