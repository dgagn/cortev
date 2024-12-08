use core::fmt;
use std::{
    borrow::Cow,
    fmt::{Debug, Formatter},
    time::Duration,
};

#[cfg(feature = "redis-pool")]
use deadpool_redis::Pool;
use redis::{
    aio::{ConnectionLike, ConnectionManager},
    cmd, FromRedisValue, RedisError,
};

use crate::{
    builder::BuildSession, driver::SessionError, error::SessionErrorKind, Session, SessionData,
    SessionKey,
};

use super::{generate_session_key, FromJson, SessionDriver, SessionResult, ToJson};

/// Represents the kind of Redis connection being used.
///
/// This enum provides flexibility for various use cases.
#[derive(Clone)]
pub enum RedisConnectionKind {
    /// Represents a connection pool, which allows multiple connections to Redis.
    ///
    /// This is generally slower than the connection manager and should be used only when:
    /// - Specific connection management is required, such as for isolating blocking operations.
    /// - Fine-grained control over individual connections is necessary.
    #[cfg(feature = "redis-pool")]
    Pool(Pool),
    /// Represents a multiplexed connection managed by a `ConnectionManager`.
    ///
    /// This is the recommended option for most use cases as it is more efficient
    /// and suitable for handling asynchronous workloads.
    Connection(ConnectionManager),
}

#[cfg(feature = "redis-pool")]
impl From<deadpool_redis::Pool> for RedisConnectionKind {
    /// Converts a `deadpool_redis::Pool` into a `RedisConnectionKind`.
    fn from(pool: Pool) -> Self {
        Self::Pool(pool)
    }
}

impl From<redis::aio::ConnectionManager> for RedisConnectionKind {
    /// Converts a `ConnectionManager` into a `RedisConnectionKind`.
    fn from(value: redis::aio::ConnectionManager) -> Self {
        Self::Connection(value)
    }
}

/// A driver for managing Redis-based session storage.
///
/// This struct encapsulates the connection type, session time-to-live (TTL), and optional
/// session key prefix, providing methods to interact with Redis for session-related operations.
#[derive(Debug, Clone)]
pub struct RedisDriver {
    connection_kind: RedisConnectionKind,
    ttl: Duration,
    prefix: Option<Cow<'static, str>>,
}

/// A builder for constructing a `RedisDriver`.
///
/// This builder allows configuring optional parameters such as session TTL and a key prefix.
#[derive(Debug)]
pub struct RedisDriverBuilder {
    connection_kind: RedisConnectionKind,
    ttl: Option<Duration>,
    prefix: Option<Cow<'static, str>>,
}

impl RedisDriverBuilder {
    /// Creates a new `RedisDriverBuilder` with the specified connection kind.
    pub(crate) fn new(connection_kind: RedisConnectionKind) -> Self {
        Self {
            connection_kind,
            ttl: None,
            prefix: None,
        }
    }

    /// Sets the session time-to-live (TTL) for the driver.
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Sets a prefix to be used for all session keys in Redis.
    pub fn with_prefix(mut self, prefix: impl Into<Cow<'static, str>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Builds the `RedisDriver` with the configured options.
    ///
    /// If no TTL is specified, a default TTL of 120 hours is used.
    pub fn build(self) -> RedisDriver {
        RedisDriver {
            connection_kind: self.connection_kind,
            ttl: self
                .ttl
                .unwrap_or_else(|| Duration::from_secs(60 * 60 * 120)),
            prefix: self.prefix,
        }
    }
}

/// Represents a Redis command, which can either be a pipeline or a single command.
///
/// This abstraction allows handling both types of Redis operations seamlessly.
pub(crate) enum RedisCommand<'a> {
    /// A pipeline containing multiple commands to be executed atomically.
    Pipeline(&'a mut redis::Pipeline),
    /// A single Redis command.
    Command(&'a mut redis::Cmd),
}

impl RedisDriver {
    /// Creates a new `RedisDriver` with the specified connection kind and TTL.
    pub fn new(connection_kind: RedisConnectionKind, ttl: Duration) -> Self {
        Self {
            connection_kind,
            ttl,
            prefix: None,
        }
    }

    /// Creates a `RedisDriverBuilder` to configure and construct a `RedisDriver`.
    pub fn builder<T>(connection_kind: T) -> RedisDriverBuilder
    where
        T: Into<RedisConnectionKind>,
    {
        RedisDriverBuilder::new(connection_kind.into())
    }

    /// Prepends the configured prefix to a session key, if a prefix is set.
    ///
    /// If no prefix is configured, the key is returned as-is.
    fn prefixed_key<'a>(&'a self, key: &'a str) -> Cow<'a, str> {
        if let Some(prefix) = &self.prefix {
            let mut result = String::with_capacity(prefix.len() + key.len());
            result.push_str(prefix);
            result.push_str(key);
            Cow::Owned(result)
        } else {
            Cow::Borrowed(key)
        }
    }

    /// Retries executing a Redis command, handling connection drops gracefully.
    ///
    /// This method is designed for high-reliability use cases where transient connection issues
    /// are expected and should not cause the operation to fail immediately.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, conn, cmd)))]
    async fn retry<T: FromRedisValue>(
        &self,
        mut conn: impl ConnectionLike,
        cmd: RedisCommand<'_>,
    ) -> Result<T, RedisError> {
        let mut can_retry = true;
        while can_retry {
            match cmd {
                RedisCommand::Pipeline(ref pipeline) => {
                    match pipeline.query_async::<T>(&mut conn).await {
                        Ok(value) => {
                            #[cfg(feature = "tracing")]
                            tracing::debug!("Pipeline query successful");
                            return Ok(value);
                        }
                        Err(err) if err.is_connection_dropped() => {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("Connection dropped, retrying...");
                            can_retry = false;
                        }
                        Err(err) => return Err(err),
                    }
                }
                RedisCommand::Command(ref command) => {
                    match command.query_async::<T>(&mut conn).await {
                        Ok(value) => {
                            #[cfg(feature = "tracing")]
                            tracing::debug!("Command query successful");
                            return Ok(value);
                        }
                        Err(err) if err.is_connection_dropped() => {
                            #[cfg(feature = "tracing")]
                            tracing::warn!("Connection dropped, retrying...");
                            can_retry = false;
                        }
                        Err(err) => return Err(err),
                    }
                }
            }
        }
        #[cfg(feature = "tracing")]
        tracing::error!("Retry loop exited without success or error");

        // Unreachable in theory
        Err(RedisError::from((
            redis::ErrorKind::IoError,
            "Retry loop exited without success or error",
        )))
    }

    /// Executes a Redis command and returns the result.
    ///
    /// Automatically selects the appropriate connection type based on the `RedisConnectionKind`
    /// configuration.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, cmd)))]
    async fn query<T: FromRedisValue>(&self, cmd: RedisCommand<'_>) -> SessionResult<T> {
        match &self.connection_kind {
            #[cfg(feature = "redis-pool")]
            RedisConnectionKind::Pool(pool) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Getting a connection from the pool...");

                let connection = pool.get().await.map_err(SessionError::AcquireConnection)?;

                let value = self.retry::<T>(connection, cmd).await?;

                Ok::<T, SessionError>(value)
            }
            RedisConnectionKind::Connection(connection) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Getting a connection from the connection manager...");

                let connection = connection.clone();
                let value = self.retry::<T>(connection, cmd).await?;

                Ok::<T, SessionError>(value)
            }
        }
    }
}

impl SessionDriver for RedisDriver {
    /// Reads a session from Redis using the specified key.
    ///
    /// If the session exists, it updates the key's TTL and returns the session.
    /// If the session does not exist, `Ok(None)` is returned.
    ///
    /// # Errors
    /// Returns a `SessionError` if reading from Redis fails or if deserialization of the session
    /// data fails.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    async fn read(&self, key: SessionKey) -> SessionResult<Option<Session>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Reading the session");

        let prefixed_key = self.prefixed_key(&key);

        let mut command = cmd("GETEX");
        let command = command.arg(&prefixed_key).arg("EX").arg(self.ttl.as_secs());

        let command = RedisCommand::Command(command);
        let value: Option<String> =
            self.query(command)
                .await
                .map_err(|source| SessionError::SessionKindError {
                    source: Box::new(source),
                    key: key.clone(),
                    kind: SessionErrorKind::Read,
                })?;

        if let Some(value) = value {
            let session = SessionData::from_json(&value)?;
            let session = Session::builder(key).with_data(session).build();

            #[cfg(feature = "tracing")]
            tracing::debug!("Session read successfully");

            Ok(Some(session))
        } else {
            #[cfg(feature = "tracing")]
            tracing::warn!("Session not found");

            Ok(None)
        }
    }

    /// Writes a session to Redis with the specified key and data.
    ///
    /// The key's TTL is set to the driver's configured TTL. The session data is serialized
    /// before being written to Redis.
    ///
    /// # Errors
    /// Returns a `SessionError` if writing to Redis fails or if the session data cannot be serialized.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn write(&self, key: SessionKey, data: SessionData) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Writing session");

        let prefixed_key = self.prefixed_key(&key);

        let data = data
            .to_json()
            .map_err(|source| SessionError::SessionKindError {
                source: Box::new(source),
                key: key.clone(),
                kind: SessionErrorKind::Write,
            })?;

        let mut command = cmd("SET");
        let command = command
            .arg(&prefixed_key)
            .arg(data)
            .arg("EX")
            .arg(self.ttl.as_secs());

        let command = RedisCommand::Command(command);
        let _: () = self
            .query(command)
            .await
            .map_err(|source| SessionError::SessionKindError {
                source: Box::new(source),
                key: key.clone(),
                kind: SessionErrorKind::Write,
            })?;

        #[cfg(feature = "tracing")]
        tracing::info!("Session written successfully");

        Ok(key)
    }

    /// Deletes a session from Redis with the specified key.
    ///
    /// # Errors
    /// Returns a `SessionError` if deleting the session from Redis fails.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    async fn destroy(&self, key: SessionKey) -> SessionResult<()> {
        let prefixed_key = self.prefixed_key(&key);
        let mut command = cmd("DEL");
        let command = command.arg(&prefixed_key);
        let command = RedisCommand::Command(command);
        let _: () = self
            .query(command)
            .await
            .map_err(|source| SessionError::SessionKindError {
                source: Box::new(source),
                key: key.clone(),
                kind: SessionErrorKind::Destroy,
            })?;

        #[cfg(feature = "tracing")]
        tracing::info!("Session destroyed");
        Ok(())
    }

    /// Regenerates a session by replacing its key while preserving its data.
    ///
    /// The old session key is deleted, and a new session key is generated and associated
    /// with the session data.
    ///
    /// # Errors
    /// Returns a `SessionError` if updating Redis fails or if the session data cannot be serialized.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn regenerate(
        &self,
        old_key: SessionKey,
        data: SessionData,
    ) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Regenerating session");
        let old_prefixed_key = self.prefixed_key(&old_key);

        let data = data.to_json()?;
        let new_key = generate_session_key();
        let prefixed_new_key = self.prefixed_key(&new_key);
        let mut pipeline = redis::pipe();
        pipeline.set_ex(&prefixed_new_key, data, self.ttl.as_secs());
        pipeline.del(&old_prefixed_key);
        pipeline.ignore();
        let command = RedisCommand::Pipeline(&mut pipeline);

        let _: () = self
            .query(command)
            .await
            .map_err(|source| SessionError::SessionKindError {
                source: Box::new(source),
                key: old_key.clone(),
                kind: SessionErrorKind::Regenerate,
            })?;

        let session_key = SessionKey::from(new_key);

        #[cfg(feature = "tracing")]
        tracing::info!("Session regenerated successfully to {:?}", session_key);

        Ok(session_key)
    }

    /// Invalidates a session by replacing its key and deleting the old session data.
    ///
    /// The old session key is deleted, and a new session key is generated and associated
    /// with the session data.
    ///
    /// # Errors
    /// Returns a `SessionError` if updating Redis fails or if the session data cannot be serialized.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn invalidate(&self, key: SessionKey, data: SessionData) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Invalidating session...");

        let prefixed_key = self.prefixed_key(&key);

        let data = data.to_json()?;
        let new_key = generate_session_key();
        let prefixed_new_key = self.prefixed_key(&new_key);
        let mut pipeline = redis::pipe();
        pipeline.del(&prefixed_key);
        pipeline.set_ex(&prefixed_new_key, data, self.ttl.as_secs());
        pipeline.ignore();

        let command = RedisCommand::Pipeline(&mut pipeline);
        let _: () = self
            .query(command)
            .await
            .map_err(|source| SessionError::SessionKindError {
                source: Box::new(source),
                key: key.clone(),
                kind: SessionErrorKind::Invalidate,
            })?;

        let session_key = SessionKey::from(new_key);

        #[cfg(feature = "tracing")]
        tracing::info!("Session invalidated successfully to {:?}", session_key);

        Ok(session_key)
    }

    /// Returns the session time-to-live (TTL) for this driver.
    fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl Debug for RedisConnectionKind {
    /// Provides a debug-friendly string representation of the connection kind.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "redis-pool")]
            RedisConnectionKind::Pool(_) => write!(f, "Pool"),
            RedisConnectionKind::Connection(_) => write!(f, "Connection"),
        }
    }
}
