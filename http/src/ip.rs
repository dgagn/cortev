use std::net::IpAddr;

use axum::{extract::connect_info::Connected, serve::IncomingStream};
use ipnet::IpNet;

#[derive(Debug, Clone)]
pub struct TrustedProxies {
    proxies: Vec<IpNet>,
}

impl TrustedProxies {
    pub fn new() -> Self {
        Self { proxies: vec![] }
    }

    pub fn is_trusted(&self, ip: &IpAddr) -> bool {
        self.proxies.iter().any(|proxy| proxy.contains(ip))
    }
}

impl Default for TrustedProxies {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClientInfo {
    ip: IpAddr,
}

impl ClientInfo {
    pub fn new(ip: IpAddr) -> Self {
        Self { ip }
    }

    pub fn ip(&self) -> &IpAddr {
        &self.ip
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ClientIp {
    ip: IpAddr,
}

impl Connected<IncomingStream<'_>> for ClientInfo {
    fn connect_info(stream: IncomingStream<'_>) -> Self {
        ClientInfo {
            ip: stream.remote_addr().ip().to_canonical(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_ip() {
        let ipv4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    }
}
