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

/// The endpoints a server bound at `bound` (`host:port`) advertises. A bind is what the
/// socket was asked for; an endpoint is what another machine can dial, and the two differ
/// exactly when the bind is unspecified — nobody can reach `0.0.0.0`, so that bind is
/// expanded into the interfaces it actually listens on. A bind this cannot parse is
/// passed through unchanged, because a name this process cannot resolve may still be one
/// its peers can.
///
/// ```
/// use majordomus_cli::mesh::address::advertised_endpoints;
///
/// assert_eq!(advertised_endpoints("127.0.0.1:8742"), ["127.0.0.1:8742"]);
///
/// // the unspecified bind never advertises itself, and keeps the port it was given
/// let wide = advertised_endpoints("0.0.0.0:8742");
/// assert!(wide.iter().all(|e| e.ends_with(":8742") && !e.starts_with("0.0.0.0")));
/// assert!(!wide.is_empty(), "loopback stands in when no interface is up");
///
/// // a host this process cannot parse is still a host its peers may know
/// assert_eq!(advertised_endpoints("gateway.local:8742"), ["gateway.local:8742"]);
/// ```
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

/// Whether every endpoint is a loopback address: reachable from this machine alone. A
/// runtime in that state is not broken and is not on the mesh either, which is a
/// distinction the doctor has to be able to draw before it blames the network for a
/// cooperation that was never reachable. One routable endpoint is enough, so this asks of
/// all of them rather than of any.
///
/// ```
/// use majordomus_cli::mesh::address::loopback_only;
///
/// assert!(loopback_only(&["127.0.0.1:8742".into(), "localhost:8742".into()]));
/// assert!(!loopback_only(&["127.0.0.1:8742".into(), "10.0.0.2:8742".into()]));
/// ```
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

/// The up, non-loopback IPv4 interfaces of this machine, ranked, with their names. What is
/// left out is what a remote runtime could not dial anyway: an interface that is down,
/// loopback, unspecified, or link-local. The order is the advertising preference — a
/// physical or overlay interface before a container or VM bridge — because only the first
/// [`MAX_ENDPOINTS`] survive into an advertisement and a bridge address is the one a peer
/// is least likely to reach.
///
/// ```
/// use majordomus_cli::mesh::address::interfaces;
///
/// // whatever this machine has, every address here is one a peer could be told to dial
/// for (name, ip) in interfaces() {
///     assert!(!ip.is_loopback() && !ip.is_unspecified(), "{name} advertises {ip}");
/// }
/// ```
#[cfg(unix)]
pub fn interfaces() -> Vec<(String, Ipv4Addr)> {
    let mut out: std::collections::BTreeMap<(u8, String, String), (String, Ipv4Addr)> =
        std::collections::BTreeMap::new();
    // SAFETY: getifaddrs hands back a linked list this function walks read-only and frees
    // exactly once; every pointer is checked for null before it is read, and a sockaddr
    // is reinterpreted as sockaddr_in only when its family says AF_INET.
    unsafe {
        let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut head) != 0 {
            return Vec::new();
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
                    // Keyed by the declared preference of the interface, then by its name
                    // and address: the map holds the order, so nothing here compares.
                    out.insert((rank(&name), name.clone(), ip.to_string()), (name, ip));
                }
            }
            cursor = entry.ifa_next;
        }
        libc::freeifaddrs(head);
    }
    out.into_values().collect()
}

/// No interface enumeration off Unix: `getifaddrs` is the only enumeration this crate
/// carries, so on any other platform the bound address is all there is to advertise. The
/// empty answer is not a failure — it makes a server bound to every interface fall back to
/// advertising loopback, which is honest about what a peer can reach rather than guessing
/// an address that may not exist.
///
/// ```
/// use majordomus_cli::mesh::address::{advertised_endpoints, interfaces};
///
/// assert!(interfaces().is_empty(), "nothing is enumerated off Unix");
/// assert_eq!(advertised_endpoints("0.0.0.0:8742"), ["127.0.0.1:8742"]);
/// ```
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
