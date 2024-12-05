use crate::SessionData;

use super::{key::SessionKey, state::SessionState, Session};

/// Marker type indicating that the session data has been provided.
#[derive(Debug)]
pub struct WithData;

/// Marker type indicating that the session data has not yet been provided.
#[derive(Debug)]
pub struct NoData;

/// A builder for constructing `Session` objects.
///
/// The `SessionBuilder` uses a type-state pattern to enforce that certain
/// steps, such as providing session data, are completed before calling `build`.
///
/// - When `SessionBuilder` is in the `NoData` state, session data has not yet been provided.
/// - When `SessionBuilder` transitions to the `WithData` state, it is ready to build the `Session`.
///
/// # Type Parameters
/// - `State`: A marker type (`NoData` or `WithData`) representing the current state of the builder.
#[derive(Debug)]
pub struct SessionBuilder<State = NoData> {
    key: SessionKey,
    data: Option<SessionData>,
    state: std::marker::PhantomData<State>,
}

impl SessionBuilder {
    pub fn new<K: Into<SessionKey>>(key: K) -> Self {
        Self {
            key: key.into(),
            data: None,
            state: Default::default(),
        }
    }
}

impl SessionBuilder<NoData> {
    pub fn with_data<T: Into<SessionData>>(self, data: T) -> SessionBuilder<WithData> {
        SessionBuilder {
            key: self.key,
            data: Some(data.into()),
            state: std::marker::PhantomData::<WithData>,
        }
    }
}

impl SessionBuilder<WithData> {
    pub fn build(self) -> Session {
        Session {
            key: self.key,
            data: self.data.unwrap(),
            state: SessionState::Unchanged,
        }
    }
}

pub trait BuildSession {
    fn builder<K: Into<SessionKey>>(key: K) -> SessionBuilder;
}

impl BuildSession for Session {
    fn builder<K: Into<SessionKey>>(key: K) -> SessionBuilder {
        SessionBuilder::new(key)
    }
}
