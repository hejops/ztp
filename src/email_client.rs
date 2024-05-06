use std::time::Duration;

use reqwest::Client;
use reqwest::Url;
use secrecy::ExposeSecret;
use secrecy::Secret;
use serde::Serialize;

use crate::domain::SubscriberEmail;

/// An email client that should be agnostic with choice of email provider. We
/// use MailChimp since I don't have an email I can use with Postmark.
//
// https://github.com/LukeMathWalker/zero-to-production/issues/176#issuecomment-1490392528
pub struct EmailClient {
    /// The client that actually communicates with the REST API
    http_client: Client,
    /// This will depend on email provider
    base_url: String,
    sender: SubscriberEmail,
    /// API key from the email provider
    authorization_token: Secret<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")] // PascalCase is required (?) by Postmark
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

// establishing a HTTP connection is expensive, so if multiple requests are to
// be sent to the same server, the connection should be reused. this can be done
// by keeping `Client` at the top-level (App) and referencing (extracting) it
// from the App
//
// `Client::clone`

impl EmailClient {
    /// `timeout` is not exposed at all, so it must be set via configuration. It
    /// is only overridden for tests (to use a small value).
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        timeout: Duration,
    ) -> Self {
        Self {
            // enforce client-wide timeout
            http_client: Client::builder()
                // .timeout(Duration::from_secs(5))
                .timeout(timeout)
                .build()
                .unwrap(),
            base_url,
            sender,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        // SMTP and REST can be used to send email; REST is usually easier to set up,

        // derived from Postmark docs: https://postmarkapp.com/developer/user-guide/send-email-with-api#send-a-single-email
        // mailchimp doesn't seem to have an exact equivalent, so we roll with it for
        // now until we inevitably run into problems
        // https://mailchimp.com/developer/marketing/api/campaigns/
        // "Send test email"
        // POST /campaigns/{campaign_id}/actions/test
        let url = format!("{}/email", self.base_url);
        let url = Url::parse(&url).unwrap();
        println!("{:?}", url);

        let body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            html_body: html_content,
            text_body: text_content,
        };

        // `.json` accepts structs (which implement `Serialize`), and also sets the
        // appropriate `Content-Type` header; `.body` doesn't
        let builder = self
            .http_client
            .post(url)
            // on Postmark this is "X-Postmark-Server-Token"
            // https://mailchimp.com/developer/transactional/guides/send-first-email/#send-your-first-email
            .header("key", self.authorization_token.expose_secret())
            .json(&body)
            .send()
            .await? // Err type must be `reqwest::Error`
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use claims::assert_err;
    use claims::assert_ok;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::Paragraph;
    use fake::faker::lorem::en::Sentence;
    use fake::Fake;
    use fake::Faker;
    use secrecy::Secret;
    use serde_json::Value;
    use wiremock::matchers::any;
    use wiremock::matchers::header;
    use wiremock::matchers::header_exists;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::Match;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;

    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;

    struct SendEmailBodyMatcher;
    impl Match for SendEmailBodyMatcher {
        fn matches(
            &self,
            request: &wiremock::Request,
        ) -> bool {
            let parsed: Value = match serde_json::from_slice(&request.body) {
                Ok(p) => p,
                Err(_) => return false,
            };

            for key in ["From", "To", "Subject", "HtmlBody", "TextBody"] {
                if parsed.get(key).is_none() {
                    return false;
                }
            }
            true
        }
    }

    // random inputs are used (over hardcoded values) to signify that these inputs
    // don't matter
    fn email() -> SubscriberEmail { SubscriberEmail::parse(SafeEmail().fake()).unwrap() }
    fn subject() -> String { Sentence(1..2).fake() }
    fn content() -> String { Paragraph(1..2).fake() }

    fn email_client(url: String) -> EmailClient {
        EmailClient::new(
            url,
            email(),
            Secret::new(Faker.fake()),
            Duration::from_millis(200),
        )
    }

    // requires `rt` feature (?)
    #[tokio::test]
    async fn send_email_returns_200() {
        // start mock server on random port. its base_url, accessed by `.uri()`, is used
        // to init the email_client
        let mock_server = MockServer::start().await;
        let sender = email_client(mock_server.uri());

        // must be declared before .send_email (which is somewhat unintuitive). this
        // explains the use of `await`
        Mock::given(
            // // respond to any request with 200; restrictions can be imposed
            // any(),
            header_exists("key"),
        )
        .and(header("Content-Type", "application/json"))
        .and(path("/email"))
        .and(method("POST"))
        .and(SendEmailBodyMatcher)
        .respond_with(ResponseTemplate::new(200))
        .expect(1) // the actual assertion: expect email_client to receive 1 request
        .mount(&mock_server)
        .await;

        // mock's test output isn't terribly helpful; it doesn't show expected/actual
        // result

        assert_ok!(
            sender
                .send_email(email(), &subject(), &content(), &content())
                .await
        );
    }

    #[tokio::test]
    async fn send_email_returns_500() {
        let mock_server = MockServer::start().await;
        let sender = email_client(mock_server.uri());

        Mock::given(any()) // respond to any request
            .respond_with(ResponseTemplate::new(500)) // simulate a 'server error'
            .expect(1)
            .mount(&mock_server)
            .await;

        assert_err!(
            sender
                .send_email(email(), &subject(), &content(), &content())
                .await
        );
    }

    #[tokio::test]
    async fn send_email_timeout() {
        let mock_server = MockServer::start().await;
        let sender = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1800))) // simulate timeout
            .expect(1)
            .mount(&mock_server)
            .await;

        assert_err!(
            sender
                .send_email(email(), &subject(), &content(), &content())
                .await
        );
    }
}
