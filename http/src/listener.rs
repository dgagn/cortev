use std::{ops::Deref, os::fd::FromRawFd};

use thiserror::Error;
use tokio::net::{TcpListener, ToSocketAddrs};

/// A listener that supports systemd socket activation and fallback local binding.
#[derive(Debug)]
pub struct SocketListener {
    listener: TcpListener,
}

impl SocketListener {
    pub async fn new<T>(bind_addr: T) -> Result<Self, Error>
    where
        T: ToSocketAddrs,
    {
        if let Ok(listen_fds) = std::env::var("LISTEN_FDS") {
            let listen_fds: i32 = listen_fds.parse()?;

            if listen_fds != 1 {
                return Err(Error::UnexpectedListenFds(listen_fds as usize));
            }

            // Safety: the file descriptor is valid because systemd guarantees it.
            let raw_fd = 3;
            let std_listener = unsafe { std::net::TcpListener::from_raw_fd(raw_fd) };
            std_listener.set_nonblocking(true)?;

            let listener = TcpListener::from_std(std_listener)?;

            Ok(Self { listener })
        } else {
            let listener = TcpListener::bind(bind_addr).await?;
            Ok(Self { listener })
        }
    }

    pub fn into_inner(self) -> TcpListener {
        self.listener
    }
}

impl Deref for SocketListener {
    type Target = TcpListener;

    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("the listen file descriptors are invalid")]
    InvalidListenFds(#[from] std::num::ParseIntError),

    #[error("expected exactly one listen file descriptor")]
    UnexpectedListenFds(usize),

    #[error("failed to bind to address")]
    Bind(#[from] std::io::Error),
}
