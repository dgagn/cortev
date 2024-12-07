use crate::policy::EncryptionCookiePolicy;

#[derive(Debug)]
pub struct CookieJarBuilder {
    jar: cookie::CookieJar,
    key: cookie::Key,
    encryption_policy: Option<EncryptionCookiePolicy>,
}

impl CookieJarBuilder {
    pub fn new(key: cookie::Key) -> Self {
        Self {
            jar: cookie::CookieJar::new(),
            key,
            encryption_policy: None,
        }
    }

    pub fn with_encryption_policy(mut self, policy: EncryptionCookiePolicy) -> Self {
        self.encryption_policy = Some(policy);
        self
    }

    pub fn build(self) -> crate::CookieJar {
        crate::CookieJar {
            jar: self.jar,
            // Unwrapping is safe because we know that the key is always present
            key: self.key.into(),
            encryption_policy: self.encryption_policy.unwrap_or_default().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{builder::CookieJarBuilder, CookieMap, EncryptionCookiePolicy};

    #[test]
    fn test_builder() {
        let key = cookie::Key::generate();
        let builder = CookieJarBuilder::new(key);
        let jar = builder.build();
        assert_eq!(
            jar.encryption_policy,
            EncryptionCookiePolicy::default().into()
        );
    }

    #[test]
    fn test_builder_with_encryption_policy() {
        let key = cookie::Key::generate();
        let policy = EncryptionCookiePolicy::Exclusion(CookieMap::new());
        let builder = CookieJarBuilder::new(key).with_encryption_policy(policy.clone());
        let jar = builder.build();
        assert_eq!(jar.encryption_policy, policy.into());
    }
}
