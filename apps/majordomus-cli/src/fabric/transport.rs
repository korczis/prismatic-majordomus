//! Which local transport this build has, and whether a socket path fits it.
//!
//! The local channel is a Unix-domain socket on every Unix target. A named pipe is the
//! Windows channel ADR 0199 names, and this build does not carry one yet, so a Windows
//! build answers [`RefusalCode::TransportUnavailable`] instead of pretending to listen.
//!
//! A socket path is bounded by the platform's socket address, not by the filesystem:
//! 104 bytes on macOS and the BSDs, 108 on Linux, NUL included. A runtime directory deep
//! enough to exceed that is common on macOS (`$TMPDIR` is long), so the bound is checked by
//! the platform's own address constructor before anything is bound, and the refusal says
//! so ([`RefusalCode::SocketPathInvalid`]) instead of surfacing as an `EINVAL` from `bind`.

use std::path::Path;

use super::refusal::{Refusal, RefusalCode};

/// Whether this build can carry the local channel.
///
/// ```
/// use majordomus_cli::fabric::transport::local_transport;
/// assert_eq!(local_transport().is_ok(), cfg!(unix));
/// ```
pub fn local_transport() -> Result<(), Refusal> {
    #[cfg(unix)]
    {
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(Refusal::new(
            RefusalCode::TransportUnavailable,
            "this build has no local channel on this platform; the named pipe is not built yet",
        ))
    }
}

/// Whether a socket can be bound at `path`: absolute, and short enough for the platform's
/// socket address.
///
/// ```
/// use majordomus_cli::fabric::transport::check_socket_path;
/// use std::path::Path;
/// # #[cfg(unix)] {
/// assert!(check_socket_path(Path::new("/run/user/1000/majordomus/host.sock")).is_ok());
/// assert!(check_socket_path(&Path::new("/").join("d".repeat(200))).is_err());
/// # }
/// ```
pub fn check_socket_path(path: &Path) -> Result<(), Refusal> {
    if !path.is_absolute() {
        return Err(Refusal::new(
            RefusalCode::SocketPathInvalid,
            "a socket path is absolute, so that no working directory changes which socket it names",
        ));
    }
    fits(path)
}

#[cfg(unix)]
fn fits(path: &Path) -> Result<(), Refusal> {
    std::os::unix::net::SocketAddr::from_pathname(path)
        .map(|_| ())
        .map_err(|e| {
            Refusal::new(
                RefusalCode::SocketPathInvalid,
                format!(
                    "a socket path of {} bytes does not fit this platform's socket address: {e}",
                    path.as_os_str().len()
                ),
            )
        })
}

#[cfg(not(unix))]
fn fits(_path: &Path) -> Result<(), Refusal> {
    local_transport()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn the_bound_is_the_platforms_and_is_found_without_binding() {
        // sun_path is 104 bytes on macOS and the BSDs and 108 on Linux, NUL included
        let limit = if cfg!(target_os = "linux") { 107 } else { 103 };
        let at = |len: usize| Path::new("/").join("s".repeat(len - 1));
        assert!(check_socket_path(&at(limit)).is_ok(), "{limit} bytes fit");
        let refused = check_socket_path(&at(limit + 1)).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::SocketPathInvalid);
        assert!(
            refused.message().contains(&format!("{} bytes", limit + 1)),
            "{refused}"
        );
        assert!(!at(limit).exists(), "nothing was bound");
    }

    #[test]
    fn a_unix_build_has_its_local_channel() {
        assert!(local_transport().is_ok());
    }

    #[test]
    fn a_relative_socket_path_is_refused() {
        let refused = check_socket_path(Path::new("host.sock")).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::SocketPathInvalid);
        assert!(refused.message().contains("absolute"));
    }
}
