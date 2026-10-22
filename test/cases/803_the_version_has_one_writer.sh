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
. "$ROOT/test/lib.sh"

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
