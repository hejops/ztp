use reqwest::Client;

use crate::domain::SubscriberEmail;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
}

// establishing a HTTP connection is expensive, so if multiple requests are to
// be sent to the same server, the connection should be reused. this can be done
// by keeping `Client` at the top-level (App) and referencing (extracting) it
// from the App
//
// `Client::clone`

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
    ) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            sender,
        }
    }
    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), String> {
        todo!()
    }
}
