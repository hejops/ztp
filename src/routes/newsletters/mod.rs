mod get;
mod post;
pub use get::*;
pub use post::*;

// Parse headers of a HTTP request. This does not actually validate any user
// credentials; for that, see `validate_credentials`.
// fn basic_authentication(headers: &HeaderMap) -> Result<Credentials,
// anyhow::Error> {     // authentication methods fall in three categories:
// passwords / objects /     // biometrics. because there are drawbacks
// associated with each, multi-factor     // authentication is recommended
//
//     // spec: RFCs 2617, 7617
//     // - correct header ("Authorization")
//     // - correct realm ("publish")
//     // - correct username/password
//
//     let encoded = headers
//         .get("Authorization")
//         .context("No Authorization header")?
//         .to_str()
//         .context("Invalid str")?
//         .strip_prefix("Basic ")
//         .context("Authorization scheme was not 'Basic'")?;
//
//     let decoded = base64::engine::general_purpose::STANDARD
//         .decode(encoded)
//         .context("Failed to decode base64")?;
//     let decoded = String::from_utf8(decoded).context("Invalid str")?;
//
//     let mut creds = decoded.splitn(2, ':');
//
//     let username = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No username"))?
//         .to_string();
//
//     let password = creds
//         .next()
//         .ok_or_else(|| anyhow::anyhow!("No password"))?
//         .to_string();
//     let password = Secret::new(password);
//
//     Ok(Credentials { username, password })
// }
