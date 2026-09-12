# majordomus-covers: doctor
# The model catalogue through the command line: one declaration in share/models.yaml,
# rendered by `models list` with credential presence (never a value), and routed by
# `models route` with the reasons attached — the selected model's why, and the first
# failing check for every excluded one. An empty catalogue is an answer, and text and
# json render the same decision.
#
# The crate's own suite (apps/majordomus-cli/src/models/mod.rs) covers the routing
# semantics — preference order, check order, overrides, diagnostics — in depth. What
# is here is the operator's path across the share declaration and the two commands.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

R="$T/repo"
fixture_repo "$R" >/dev/null
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null

mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# ---------------------------------------------------------------- 1. the catalogue is one declaration
cat > "$R/share/models.yaml" <<'YAML'
version: 1
vendors:
  - id: acme
    title: Acme
    credential_env: CASE_ACME_KEY
models:
  - id: acme-large
    vendor: acme
    native_id: acme-large-1
    aliases: [large]
    context_window: 200000
    capabilities: [text, tools, vision]
  - id: acme-mini
    vendor: acme
    native_id: acme-mini-1
    context_window: 32000
    capabilities: [text]
YAML
expect_exit 0 mj "$R" models list
expect_grep "2 model"
expect_grep "acme-large"
expect_grep "credential not configured"

# presence only: setting the variable flips the report, and no value ever prints
CASE_ACME_KEY="a-secret-value" expect_exit 0 mj "$R" models list
expect_grep "credential configured"
if printf '%s' "$LAST_OUT" | grep -q "a-secret-value"; then
  echo "    the credential value leaked into the listing"; exit 1
fi

# ---------------------------------------------------------------- 2. routing explains itself
expect_exit 0 mj "$R" models route --require vision
expect_grep "selected   acme-large"
expect_grep "excluded   acme-mini"
expect_grep "missing capability: vision"

# an override that cannot do the work is refused with the reason, not silently taken
expect_exit 0 mj "$R" models route --model acme-mini --require vision
expect_grep "selected   nothing qualifies"

# ---------------------------------------------------------------- 3. json and text are one answer
expect_exit 0 mj "$R" models route --require vision --format json
expect_grep '"selected": "acme-large"'

# ---------------------------------------------------------------- 4. an empty catalogue is an answer
rm "$R/share/models.yaml"
expect_exit 0 mj "$R" models list
expect_grep "0 model"
expect_exit 0 mj "$R" models route
expect_grep "selected   nothing qualifies"
