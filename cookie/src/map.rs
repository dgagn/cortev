use std::{borrow::Cow, collections::HashMap};

use crate::CookieKind;

pub type CookieKey = Cow<'static, str>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CookieMap {
    data: HashMap<CookieKey, CookieKind>,
}

impl CookieMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<I: Into<CookieKey>>(&mut self, key: I, kind: CookieKind) -> &mut Self {
        self.data.insert(key.into(), kind);
        self
    }

    pub fn get(&self, key: &CookieKey) -> Option<CookieKind> {
        self.data.get(key).copied()
    }

    pub fn has(&self, key: &CookieKey) -> bool {
        self.data.contains_key(key)
    }
}
