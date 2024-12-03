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
            key: self.key,
            encryption_policy: self.encryption_policy.unwrap_or_default(),
        }
    }
}
