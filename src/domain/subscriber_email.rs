use validator::ValidateEmail;

#[derive(Debug)]
/// This struct exists only for email parsing and can be used for both senders
/// and recipients.
pub struct SubscriberEmail(String);

impl SubscriberEmail {
    pub fn parse(email: String) -> Result<Self, String> {
        ValidateEmail::validate_email(&email)
            // https://stackoverflow.com/a/65012849
            .then_some(Self(email.clone()))
            .ok_or(format!("Invalid email: {email:?}"))
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str { &self.0 }
}

#[cfg(test)]
mod tests {
    use claims::assert_err;
    use fake::faker::internet::en::SafeEmail;
    use fake::Fake;
    use quickcheck::Arbitrary;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    use crate::domain::SubscriberEmail;

    // property-based testing greatly increases the range of inputs to be validated,
    // but is still not exhaustive. `fake` is used to generate random emails,
    // `quickcheck` is used to test random inputs in bulk (100 by default)

    // #[test]
    // fn email_ok() {
    //     // assert_ok!(SubscriberEmail::parse("john@foo.com".to_string()));
    //     assert_ok!(SubscriberEmail::parse(SafeEmail().fake()));
    // }

    #[derive(Clone, Debug)]
    struct TestEmail(pub String);

    // `quickcheck::Gen` used to be directly compatible with `fake`, now it isn't,
    // because it doesn't implement `RngCore`
    // https://github.com/LukeMathWalker/zero-to-production/issues/34#issuecomment-1552385593
    impl Arbitrary for TestEmail {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut rng = StdRng::seed_from_u64(u64::arbitrary(g));
            Self(SafeEmail().fake_with_rng(&mut rng))
        }
    }

    // the type passed to `quickcheck` must implement `Arbitrary`. in this case,
    // `String` implements it, but the range of inputs is too large; we need the
    // inputs to look mostly like email addresses
    #[quickcheck_macros::quickcheck]
    fn email_ok(email: TestEmail) -> bool { SubscriberEmail::parse(email.0).is_ok() }

    #[test]
    fn empty() {
        assert_err!(SubscriberEmail::parse("".to_string()));
    }

    #[test]
    fn no_at() {
        assert_err!(SubscriberEmail::parse("johnfoo.com".to_string()));
    }

    #[test]
    fn no_subject() {
        assert_err!(SubscriberEmail::parse("@foo.com".to_string()));
    }
}
