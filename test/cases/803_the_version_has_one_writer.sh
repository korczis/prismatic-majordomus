# majordomus-covers: none
# The version has one writer, and the automatic advance goes through it (ADR 0085, ADR 0106).
#
# `release bump` and `release advance` choose a version two ways and write it one way:
# release::version::write, called from one place, commands/release.rs's write_and_verify. A
# second writer is the defect ADR 0085 removed — a hook, a workflow or a merge script that
# edits the manifest, the lock or share/version.txt itself — and an automatic lifecycle
# (finish, the integration drain, a provider hook) is exactly where one would be added for
# convenience. This case holds both halves, on the real tree and on a fixture that plants
# each kind of second writer, so the check is seen failing before it is trusted.
#
#   Rust     release::version::write is called from commands/release.rs and nowhere else
#   adapters no hook, script, recipe or workflow rewrites the manifest, the lock or the
#            projection with sed -i, perl, cargo set-version, tee or a redirection
#   driver   the merge driver (release/reconcile.rs) changes the version line only through
#            the writer's pure rewrites (version::rewrite_manifest, version::rewrite_lock),
#            names nothing else of release::version but the readers and the two paths, and
#            writes no file but the %A git hands it and its own scratch copies — so a direct
#            write of the manifest or the lock added there is caught
. "$ROOT/test/lib.sh"
S_DRIVER="$(mktemp "${TMPDIR:-/tmp}/mj803.XXXXXX")"; trap 'rm -f "$S_DRIVER"' EXIT

# one_writer <root>: prints every violation, exits 1 when there is one
one_writer() {
  local root="$1" bad=0 hits
  hits="$(git -C "$root" grep -nE '(release::)?version::write\(' -- apps \
          | grep -vE '^apps/majordomus-cli/src/release/version\.rs:' \
          | grep -vE '^apps/majordomus-cli/src/commands/release\.rs:' || true)"
  [ -z "$hits" ] || { printf 'second Rust writer: %s\n' "$hits"; bad=1; }
  hits="$(git -C "$root" grep -nE '(Cargo\.(toml|lock)|share/version\.txt)' -- \
            lib scripts bin .githooks .claude/hooks .github/workflows justfile .just 2>/dev/null \
          | grep -E 'sed -i|perl |set-version|tee |>> *"?[$A-Za-z_/{}.-]*(Cargo\.(toml|lock)|version\.txt)|> *"?[$A-Za-z_/{}.-]*(Cargo\.(toml|lock)|version\.txt)' || true)"
  [ -z "$hits" ] || { printf 'second adapter writer: %s\n' "$hits"; bad=1; }
  driver_pure "$root" || bad=1
  return "$bad"
}

# driver_pure <root>: the merge driver's non-test code uses the writer's pure rewrites and
# nothing that writes the version files itself. Prints every violation, exits 1 on one.
driver_pure() {
  local root="$1" src="apps/majordomus-cli/src/release/reconcile.rs" body bad=0 hits f
  body="$(awk '/^#\[cfg\(test\)\]/ { exit } { printf "%d:%s\n", NR, $0 }' "$root/$src" 2>/dev/null)"
  [ -n "$body" ] || { printf 'merge driver: %s is not in the tree\n' "$src"; return 1; }
  for f in rewrite_manifest rewrite_lock; do
    grep -q "version::$f" <<<"$body" || { printf 'merge driver: %s no longer uses version::%s\n' "$src" "$f"; bad=1; }
  done
  # of release::version, only the pure rewrites, the readers, the two paths and the type
  hits="$(grep -oE 'version::[A-Za-z_]+' <<<"$body" | sort -u \
          | grep -vxE 'version::(rewrite_manifest|rewrite_lock|declared_in|locked_in|MANIFEST|LOCK|Version)' || true)"
  [ -z "$hits" ] || { printf 'merge driver: %s uses %s\n' "$src" "$(tr '\n' ' ' <<<"$hits")"; bad=1; }
  # a file is written only to the %A git hands the driver, or to its scratch copies
  hits="$(grep -E 'fs::(write|rename|copy|OpenOptions)|File::(create|options)|OpenOptions::' <<<"$body" \
          | grep -vE 'std::fs::write\(ours, |std::fs::File::create\(&p\)' || true)"
  [ -z "$hits" ] || { printf 'merge driver writes outside %%A: %s\n' "$hits"; bad=1; }
  return "$bad"
}

# the real tree has one writer
expect_exit 0 one_writer "$ROOT"

# a fixture with the same shape, then each kind of second writer planted in it
git init -q fx && git -C fx config user.email t@example.com && git -C fx config user.name t
mkdir -p fx/apps/majordomus-cli/src/release fx/apps/majordomus-cli/src/commands fx/lib fx/.github/workflows fx/.claude/hooks
echo 'pub fn write() {}' > fx/apps/majordomus-cli/src/release/version.rs
echo 'fn w() { version::write(root, to); }' > fx/apps/majordomus-cli/src/commands/release.rs
echo 'echo ok' > fx/lib/finish.sh
cat > fx/apps/majordomus-cli/src/release/reconcile.rs <<'RS'
use super::version::{self, Version};
fn rewrite(file: VersionFile, text: &str, to: &str) -> String {
    match file {
        VersionFile::Manifest => version::rewrite_manifest(text, to),
        VersionFile::Lock => version::rewrite_lock(text, to),
    }
}
fn scratch(p: &Path) { std::fs::File::create(&p); }
pub fn drive(ours: &Path, merged: &str) { std::fs::write(ours, merged); }
#[cfg(test)]
mod tests {
    fn t() { std::fs::write(&o, "x"); }
}
RS
git -C fx add -A && git -C fx commit -qm base
expect_exit 0 one_writer fx

echo 'fn advance() { crate::release::version::write(root, "1.2.0"); }' > fx/apps/majordomus-cli/src/finish_helper.rs
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'second Rust writer: apps/majordomus-cli/src/finish_helper.rs'
git -C fx rm -q --cached apps/majordomus-cli/src/finish_helper.rs && rm fx/apps/majordomus-cli/src/finish_helper.rs

echo 'sed -i "" "s/^version = .*/version = \"1.2.0\"/" apps/majordomus-cli/Cargo.toml' >> fx/lib/finish.sh
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'second adapter writer: lib/finish.sh'
git -C fx checkout -q HEAD -- lib/finish.sh

printf 'steps:\n  - run: cargo set-version --bump minor --manifest-path apps/majordomus-cli/Cargo.toml\n' > fx/.github/workflows/bump.yml
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'second adapter writer: .github/workflows/bump.yml'
git -C fx rm -q --cached .github/workflows/bump.yml && rm fx/.github/workflows/bump.yml

echo 'printf "version=1.2.0\n" > share/version.txt' > fx/.claude/hooks/majordomus-session-end
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'second adapter writer: .claude/hooks/majordomus-session-end'
git -C fx rm -q --cached .claude/hooks/majordomus-session-end && rm fx/.claude/hooks/majordomus-session-end

# the merge driver writes the manifest itself instead of the %A git handed it
R=fx/apps/majordomus-cli/src/release/reconcile.rs
cp "$R" "$S_DRIVER"
sed -i.bak 's|^pub fn drive(ours: &Path, merged: &str) { std::fs::write(ours, merged); }$|pub fn drive(root: \&Path, merged: \&str) { std::fs::write(root.join(version::MANIFEST), merged); }|' "$R"
rm -f "$R.bak"; git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'merge driver writes outside %A: .*root.join\(version::MANIFEST\)'
cp "$S_DRIVER" "$R"; git -C fx add -A
expect_exit 0 one_writer fx

# the merge driver calls the writer, or rewrites the version line its own way
printf 'fn advance(root: &Path) { version::write(root, "1.2.0"); }\n' > "$R.extra"
awk -v extra="$(cat "$R.extra")" '/^#\[cfg\(test\)\]/ { print extra } { print }' "$S_DRIVER" > "$R"; rm -f "$R.extra"
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'second Rust writer: apps/majordomus-cli/src/release/reconcile.rs'
expect_grep 'merge driver: apps/majordomus-cli/src/release/reconcile.rs uses version::write'
sed 's/version::rewrite_lock(text, to)/text.replace(\"1.1.0\", to)/' "$S_DRIVER" > "$R"
git -C fx add -A
expect_exit 1 one_writer fx
expect_grep 'merge driver: apps/majordomus-cli/src/release/reconcile.rs no longer uses version::rewrite_lock'
cp "$S_DRIVER" "$R"; git -C fx add -A
expect_exit 0 one_writer fx
