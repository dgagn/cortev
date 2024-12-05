use core::fmt;
use std::{
    borrow::Cow,
    fmt::{Debug, Formatter},
    time::Duration,
};

use anyhow::Context;
use deadpool_redis::Pool;
use redis::{cmd, AsyncCommands, FromRedisValue, RedisError};

use crate::{builder::BuildSession, driver::SessionError, Session, SessionData, SessionKey};

use super::{generate_random_key, FromJson, SessionDriver, SessionResult, ToJson};

#[derive(Clone)]
pub enum RedisConnectionKind {
    Pool(Pool),
}

impl From<deadpool_redis::Pool> for RedisConnectionKind {
    fn from(pool: Pool) -> Self {
        Self::Pool(pool)
    }
}

#[derive(Debug, Clone)]
pub struct RedisDriver {
    connection_kind: RedisConnectionKind,
    ttl: Duration,
    prefix: Option<Cow<'static, str>>,
}

#[derive(Debug)]
pub struct RedisDriverBuilder {
    connection_kind: RedisConnectionKind,
    ttl: Option<Duration>,
    prefix: Option<Cow<'static, str>>,
}

impl RedisDriverBuilder {
    pub(crate) fn new(connection_kind: RedisConnectionKind) -> Self {
        Self {
            connection_kind,
            ttl: None,
            prefix: None,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_prefix(mut self, prefix: impl Into<Cow<'static, str>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

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

pub(crate) enum RedisCommand<'a> {
    Pipeline(&'a mut redis::Pipeline),
    Command(&'a mut redis::Cmd),
}

impl RedisDriver {
    pub fn new(connection_kind: RedisConnectionKind, ttl: Duration) -> Self {
        Self {
            connection_kind,
            ttl,
            prefix: None,
        }
    }

    pub fn builder<T>(connection_kind: T) -> RedisDriverBuilder
    where
        T: Into<RedisConnectionKind>,
    {
        RedisDriverBuilder::new(connection_kind.into())
    }

    pub fn with_prefix(mut self, prefix: impl Into<Cow<'static, str>>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

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

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, conn, cmd)))]
    async fn retry<T: FromRedisValue>(
        &self,
        mut conn: impl AsyncCommands,
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

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, cmd)))]
    async fn query<T: FromRedisValue>(&self, cmd: RedisCommand<'_>) -> SessionResult<T> {
        match &self.connection_kind {
            RedisConnectionKind::Pool(pool) => {
                #[cfg(feature = "tracing")]
                tracing::debug!("Getting a connection from the pool...");
                let connection = pool.get().await.context("cannot get a connection")?;
                let value = self
                    .retry::<T>(connection, cmd)
                    .await
                    .context("cannot execute the redis command")?;
                Ok::<T, SessionError>(value)
            }
        }
    }
}

impl SessionDriver for RedisDriver {
    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    async fn read(&self, key: SessionKey) -> SessionResult<Session> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Reading the session");

        let prefixed_key = self.prefixed_key(&key);

        let mut command = cmd("GETEX");
        let command = command.arg(&prefixed_key).arg("EX").arg(self.ttl.as_secs());
        let command = RedisCommand::Command(command);
        let value: Option<String> = self
            .query(command)
            .await
            .with_context(|| format!("cannot read session from key {}", key))?;

        if let Some(value) = value {
            let session = SessionData::from_json(&value)
                .with_context(|| format!("cannot deserialize session data from key {}", key))?;
            let session = Session::builder(key).with_data(session).build();

            #[cfg(feature = "tracing")]
            tracing::debug!("Session read successfully");
            Ok(session)
        } else {
            #[cfg(feature = "tracing")]
            tracing::warn!("Session not found");
            Err(SessionError::NotFound)
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn write(&self, key: SessionKey, data: SessionData) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Writing session");

        let prefixed_key = self.prefixed_key(&key);

        let data = data
            .to_json()
            .with_context(|| format!("cannot serialize session data to key {}", key))?;

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
            .with_context(|| format!("cannot write session to key {}", key))?;

        #[cfg(feature = "tracing")]
        tracing::info!("Session written successfully");

        Ok(key)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self)))]
    async fn destroy(&self, key: SessionKey) -> SessionResult<()> {
        let prefixed_key = self.prefixed_key(&key);
        let mut command = cmd("DEL");
        let command = command.arg(&prefixed_key);
        let command = RedisCommand::Command(command);
        let _: () = self
            .query(command)
            .await
            .with_context(|| format!("cannot destroy session from key {}", key))?;

        #[cfg(feature = "tracing")]
        tracing::info!("Session destroyed");
        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn regenerate(
        &self,
        old_key: SessionKey,
        data: SessionData,
    ) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Regenerating session");
        let old_prefixed_key = self.prefixed_key(&old_key);

        let data = data
            .to_json()
            .with_context(|| format!("cannot serialize session data to key {}", old_key))?;
        let new_key = generate_random_key(64);
        let prefixed_new_key = self.prefixed_key(&new_key);
        let mut pipeline = redis::pipe();
        pipeline.set_ex(&prefixed_new_key, data, self.ttl.as_secs());
        pipeline.del(&old_prefixed_key);
        pipeline.ignore();
        let command = RedisCommand::Pipeline(&mut pipeline);

        let _: () = self
            .query(command)
            .await
            .with_context(|| format!("cannot regenerate session from key {}", old_key))?;

        let session_key = SessionKey::from(new_key);

        #[cfg(feature = "tracing")]
        tracing::info!("Session regenerated successfully to {:?}", session_key);
        Ok(session_key)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(skip(self, data)))]
    async fn invalidate(&self, key: SessionKey, data: SessionData) -> SessionResult<SessionKey> {
        #[cfg(feature = "tracing")]
        tracing::debug!("Invalidating session...");

        let prefixed_key = self.prefixed_key(&key);

        let data = data.to_json().with_context(|| {
            format!(
                "cannot serialize session data to key {} for invalidation",
                key
            )
        })?;
        let new_key = generate_random_key(64);
        let prefixed_new_key = self.prefixed_key(&new_key);
        let mut pipeline = redis::pipe();
        pipeline.del(&prefixed_key);
        pipeline.set_ex(&prefixed_new_key, data, self.ttl.as_secs());
        pipeline.ignore();

        let command = RedisCommand::Pipeline(&mut pipeline);
        let _: () = self
            .query(command)
            .await
            .with_context(|| format!("cannot invalidate session from key {}", key))?;

        let session_key = SessionKey::from(new_key);

        #[cfg(feature = "tracing")]
        tracing::info!("Session invalidated successfully to {:?}", session_key);

        Ok(session_key)
    }

    fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl Debug for RedisConnectionKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RedisConnectionKind::Pool(_) => write!(f, "Pool"),
        }
    }
}
