// #[derive(Debug)]
pub struct IdempotencyKey(String);

impl TryFrom<String> for IdempotencyKey {
    type Error = anyhow::Error;
    // String -> ik
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            anyhow::bail!("Idempotency key cannot be empty")
        }
        let max_chars = 50;
        if value.len() > max_chars {
            anyhow::bail!("Idempotency key cannot be longer than {max_chars} characters")
        }
        Ok(Self(value))
    }
}

impl AsRef<str> for IdempotencyKey {
    // ik -> &str
    fn as_ref(&self) -> &str { &self.0 }
}

impl From<IdempotencyKey> for String {
    // ik -> String
    fn from(value: IdempotencyKey) -> Self { value.0 }
}
