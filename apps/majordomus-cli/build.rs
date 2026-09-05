//! Embeds every `share/allow/<name>.txt` of the repository into the binary, so that the
//! key allow-lists this executable validates against are the same files the shell tool
//! reads, and a new allow-list is picked up by name without a code change.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share/allow");
    println!("cargo:rerun-if-changed={}", dir.display());
    let entries = fs::read_dir(&dir).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; the crate is built from inside the repository, beside share/allow",
            dir.display()
        )
    });
    let mut lists: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .filter_map(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| (s.to_string(), p.clone()))
        })
        .collect();
    lists.sort();
    let mut out = String::from(
        "/// Every `share/allow/<name>.txt` of the repository, embedded at build time.\n",
    );
    out.push_str("pub const ALLOW_LISTS: &[(&str, &str)] = &[\n");
    for (name, path) in &lists {
        println!("cargo:rerun-if-changed={}", path.display());
        let abs = path.canonicalize().expect("allow-list path resolves");
        out.push_str(&format!(
            "    ({name:?}, include_str!({:?})),\n",
            abs.display().to_string()
        ));
    }
    out.push_str("];\n");
    let dest = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("allow_lists.rs");
    fs::write(&dest, out).expect("write allow_lists.rs");
}
