//! Build metadata compiled into the executable: the target triple it was built for, the
//! profile, and the commit it was built from. All three are facts of the build and are
//! read here, once, rather than shelled out for at run time — an installed binary has no
//! repository to ask and must still be able to say what it is.
//!
//! `MAJORDOMUS_BUILD_COMMIT` overrides the commit, which is how a release pipeline records
//! the commit it checked out when the build happens outside a git work tree.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MAJORDOMUS_BUILD_COMMIT");
    println!(
        "cargo:rustc-env=MAJORDOMUS_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
    println!(
        "cargo:rustc-env=MAJORDOMUS_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into())
    );
    let commit = std::env::var("MAJORDOMUS_BUILD_COMMIT")
        .ok()
        .filter(|c| !c.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=MAJORDOMUS_COMMIT={commit}");
}
