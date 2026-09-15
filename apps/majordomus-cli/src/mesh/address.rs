//! Where this runtime can be reached: the `host:port` authorities it advertises and dials
//! back. A server bound to a specific address advertises that address. A server bound to
//! every interface (`0.0.0.0`) is reachable at each interface's address and at none
//! called `0.0.0.0`, so it advertises its up, non-loopback IPv4 interface addresses —
//! physical ones before container bridges, at most [`super::protocol::MAX_ENDPOINTS`].
//! A server bound to loopback advertises loopback, which only this machine can dial; the
//! mesh doctor says so.
//!
//! ```
//! use majordomus_cli::mesh::address::advertised_endpoints;
//! assert_eq!(advertised_endpoints("127.0.0.1:8742"), vec!["127.0.0.1:8742".to_string()]);
//! assert!(advertised_endpoints("0.0.0.0:8742").iter().all(|e| !e.starts_with("0.0.0.0")));
//! ```

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use super::protocol::MAX_ENDPOINTS;

/// The endpoints a server bound at `bound` (`host:port`) advertises.
pub fn advertised_endpoints(bound: &str) -> Vec<String> {
    let Ok(addr) = bound.parse::<SocketAddr>() else {
        return vec![bound.to_string()];
    };
    if !addr.ip().is_unspecified() {
        return vec![addr.to_string()];
    }
    let mut endpoints: Vec<String> = interfaces()
        .into_iter()
        .map(|(_, ip)| SocketAddr::new(IpAddr::V4(ip), addr.port()).to_string())
        .collect();
    endpoints.dedup();
    endpoints.truncate(MAX_ENDPOINTS);
    if endpoints.is_empty() {
        endpoints.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port()).to_string());
    }
    endpoints
}

/// Whether every endpoint is a loopback address: reachable from this machine alone.
pub fn loopback_only(endpoints: &[String]) -> bool {
    endpoints.iter().all(|e| {
        e.parse::<SocketAddr>()
            .map(|a| a.ip().is_loopback())
            .unwrap_or_else(|_| e.starts_with("localhost:"))
    })
}

/// Rank an interface: physical and overlay interfaces first, container and VM bridges
/// last — a remote runtime can dial the first, rarely the second.
fn rank(name: &str) -> u8 {
    const BRIDGES: &[&str] = &[
        "docker", "br-", "virbr", "veth", "vmnet", "bridge", "lxc", "cni",
    ];
    u8::from(BRIDGES.iter().any(|b| name.starts_with(b)))
}

/// The up, non-loopback IPv4 interfaces, ranked, with their names.
#[cfg(unix)]
pub fn interfaces() -> Vec<(String, Ipv4Addr)> {
    let mut out = Vec::new();
    // SAFETY: getifaddrs hands back a linked list this function walks read-only and frees
    // exactly once; every pointer is checked for null before it is read, and a sockaddr
    // is reinterpreted as sockaddr_in only when its family says AF_INET.
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) != 0 {
            return out;
        }
        let mut cursor = head;
        while !cursor.is_null() {
            let entry = &*cursor;
            let flags = entry.ifa_flags as libc::c_int;
            let address = entry.ifa_addr;
            if flags & libc::IFF_UP != 0
                && flags & libc::IFF_LOOPBACK == 0
                && !address.is_null()
                && libc::c_int::from((*address).sa_family) == libc::AF_INET
            {
                let sin = &*(address as *const libc::sockaddr_in);
                let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
                let name = if entry.ifa_name.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(entry.ifa_name)
                        .to_string_lossy()
                        .into_owned()
                };
                if !ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local() {
                    out.push((name, ip));
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
    }
    out.sort_by_key(|(name, _)| rank(name));
    out
}

/// No interface enumeration off Unix: the bound address is all there is to advertise.
#[cfg(not(unix))]
pub fn interfaces() -> Vec<(String, Ipv4Addr)> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_specific_bind_advertises_itself_and_the_unspecified_bind_never_advertises_zero() {
        assert_eq!(advertised_endpoints("192.168.1.5:9"), vec!["192.168.1.5:9"]);
        let wide = advertised_endpoints("0.0.0.0:8742");
        assert!(!wide.is_empty() && wide.len() <= MAX_ENDPOINTS);
        assert!(wide
            .iter()
            .all(|e| e.ends_with(":8742") && !e.starts_with("0.0.0.0")));
    }

    #[test]
    fn loopback_is_named_as_this_machine_only() {
        assert!(loopback_only(&["127.0.0.1:1".into()]));
        assert!(!loopback_only(&["127.0.0.1:1".into(), "10.0.0.2:1".into()]));
    }

    #[test]
    fn bridges_rank_after_physical_interfaces() {
        assert!(rank("docker0") > rank("eth0"));
        assert!(rank("virbr0") > rank("tailscale0"));
    }
}
