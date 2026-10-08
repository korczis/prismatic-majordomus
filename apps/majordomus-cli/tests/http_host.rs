//! The interface a local server binds: flag, then `MAJORDOMUS_HTTP_HOST`, then loopback.
//!
//! One test, in a binary of its own, because it writes this process's environment: a
//! second test beside it would race it for the variable, and a test in a shared binary
//! would hand a non-loopback host to whichever neighbour started a server at that moment.
//! `test/cases/992_the_local_bind_is_the_machines_to_name.sh` holds the same resolution
//! from outside, through the three commands that start a server.

use majordomus_cli::cli::{
    local_http_host, resolve_http_host, HostOrigin, HTTP_HOST_ENV, LOOPBACK_HOST,
};

#[test]
fn the_host_is_the_flag_then_the_variable_then_loopback() {
    // the pure resolution, every arm
    assert_eq!(
        resolve_http_host(Some("10.0.0.1"), Some("0.0.0.0")),
        ("10.0.0.1".to_string(), HostOrigin::Flag)
    );
    assert_eq!(
        resolve_http_host(None, Some(" 0.0.0.0 ")),
        ("0.0.0.0".to_string(), HostOrigin::Environment)
    );
    assert_eq!(
        resolve_http_host(None, Some("   ")),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default)
    );
    assert_eq!(
        resolve_http_host(None, None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default)
    );

    // and against the environment of this process, which is what a server reads
    std::env::remove_var(HTTP_HOST_ENV);
    assert_eq!(
        local_http_host(None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default),
        "nothing names an interface, so the bind is loopback"
    );

    std::env::set_var(HTTP_HOST_ENV, "0.0.0.0");
    assert_eq!(
        local_http_host(None),
        ("0.0.0.0".to_string(), HostOrigin::Environment),
        "the variable names the interface when no flag does"
    );
    assert_eq!(
        local_http_host(Some("127.0.0.1")),
        ("127.0.0.1".to_string(), HostOrigin::Flag),
        "the command line wins over the variable"
    );

    std::env::set_var(HTTP_HOST_ENV, "");
    assert_eq!(
        local_http_host(None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default),
        "a variable that is set and blank is unset"
    );
    std::env::remove_var(HTTP_HOST_ENV);
}
