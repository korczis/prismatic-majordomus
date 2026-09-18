//! Build metadata compiled into the executable: the target triple it was built for, the
//! profile, the commit it was built from, and the *generation* — a digest of the crate
//! sources this build read. All four are facts of the build and are read here, once,
//! rather than shelled out for at run time — an installed binary has no repository to ask
//! and must still be able to say what it is.
//!
//! `MAJORDOMUS_BUILD_COMMIT` overrides the commit, which is how a release pipeline records
//! the commit it checked out when the build happens outside a git work tree — a container
//! build is one: the image copies the crate and not the repository, so without the build
//! argument the deployed process could only say `unknown`. `MAJORDOMUS_BUILD_DIRTY`
//! (`true`/`false`) says whether that tree carried uncommitted changes; without it the flag
//! is read from git when git named the commit, and is `unknown` otherwise, because a commit
//! handed in from outside says nothing about the tree it was built from.
//!
//! The generation is what `majordomus generate` compares against the tree it is asked to
//! derive. `src/generation.rs` is compiled into this build script as well as into the
//! crate, through `#[path]` rather than a dependency: a build script cannot use the crate
//! it is building, and the digest must have exactly one definition or the two sides of the
//! comparison would drift apart the first time one of them was edited. That is also why
//! that file may use nothing but `std` and `sha2`.

use std::path::PathBuf;
use std::process::Command;

#[path = "src/generation.rs"]
mod generation;

fn main() {
    // The default — rerun when any file in the package changes — is off the moment a build
    // script emits any `rerun-if` instruction, and this one has always emitted the env line
    // below. The generation is a digest of the paths named here, so it goes stale the
    // instant they change: they have to be declared, or an edited source would leave an old
    // generation compiled in and the guard would be comparing yesterday's answer.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MAJORDOMUS_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=MAJORDOMUS_BUILD_DIRTY");
    println!(
        "cargo:rustc-env=MAJORDOMUS_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
    println!(
        "cargo:rustc-env=MAJORDOMUS_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into())
    );
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
    };
    let declared = std::env::var("MAJORDOMUS_BUILD_COMMIT")
        .ok()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty() && c != "unknown");
    let from_git = declared.is_none();
    let commit = declared
        .or_else(|| {
            git(&["rev-parse", "HEAD"])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".into());
    let dirty = match std::env::var("MAJORDOMUS_BUILD_DIRTY").ok().as_deref() {
        Some("true" | "1") => "true",
        Some("false" | "0") => "false",
        _ if from_git && commit != "unknown" => {
            match git(&["status", "--porcelain", "--untracked-files=no"]) {
                Some(out) if out.trim().is_empty() => "false",
                Some(_) => "true",
                None => "unknown",
            }
        }
        _ => "unknown",
    };
    println!("cargo:rustc-env=MAJORDOMUS_DIRTY={dirty}");
    println!("cargo:rustc-env=MAJORDOMUS_COMMIT={commit}");
    let crate_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let generation =
        generation::crate_generation(&crate_dir).unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=MAJORDOMUS_GENERATION={generation}");
}
