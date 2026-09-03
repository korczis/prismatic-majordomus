. "$ROOT/test/lib.sh"
expect_exit 0 "$MJ" init
[ -f .majordomus/policy.yaml ]; [ -f .majordomus/profiles/debugging.yaml ]
[ -f .majordomus/state/decisions.md ]; [ -d .majordomus/state/handovers ]
expect_grep 'next: majordomus update'
# refuses to overwrite
echo "custom: note" >> .majordomus/state/decisions.md
expect_exit 15 "$MJ" init
expect_grep 'already exists'
# --force rewrites policy but never state
echo "# edited" >> .majordomus/policy.yaml
expect_exit 0 "$MJ" init --force
expect_no_grep '# edited' .majordomus/policy.yaml
expect_grep 'custom: note' .majordomus/state/decisions.md
# --gitignore appends once
expect_exit 0 "$MJ" init --force --gitignore
expect_exit 0 "$MJ" init --force --gitignore
[ "$(grep -c '^.majordomus/state/$' .gitignore)" = 1 ]
# outside a git repo
outside="$(mktemp -d "${TMPDIR:-/tmp}/mj-plain.XXXXXX")"; expect_exit 2 "$MJ" --repo "$outside" init; rm -rf "$outside"
