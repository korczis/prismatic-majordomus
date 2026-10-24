# majordomus-covers: capability:quality.secret_argv
# A secret never travels on a command line (I3996, rule
# project.a-secret-never-travels-on-a-command-line): the gate `secret-argv` refuses a Rust or a
# shell command line that carries a value read from a credential source, and passes the same
# value given on standard input or in the child's environment. Fixtures only; no mc, no MinIO,
# no network, and no real credential: the one value below is a placeholder.
#
#   the I2674 shape      a secret read from a credentials file pushed onto an argv: refused,
#                        naming the file, the line and the rule
#   the fixed form       the same secret as the child's standard input: passes
#   a shell token        an argument of curl: refused; on stdin or in the environment: passes
#   a declared line      `secret-argv: <why>` on the line before: passes
#   no value leaks       the report names the binding, never the placeholder's content
#   over MCP             tools/call answers the same findings as the command line
#   this repository      its own tree passes
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
PLACEHOLDER='NOT-A-REAL-SECRET-3996'

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3996.XXXXXX")"
trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null
mkdir -p apps/x/src scripts
printf 'fn main() {}\n' > apps/x/src/main.rs
git add -A >/dev/null && git commit -qm install

gate() { # -> stdout of the gate; its exit status in $code
  code=0
  out="$("$RB" quality secret-argv "$@" 2>"$S/gate.err")" || code=$?
}

gate
[ "$code" = 0 ] || { echo "    a tree with nothing to find did not pass ($code): $out"; exit 1; }
printf '%s\n' "$out" | grep -q '^secret-argv: no secret on a command line' \
  || { echo "    no passing verdict: $out"; exit 1; }
echo "    a tree with nothing to find passes"

# ---------------------------------------------------------------- 1. the I2674 shape
cat > apps/x/src/provision.rs <<'RS'
fn identity(cred: &Path, report: &mut Report) {
    let Some(secret) = secret_of(cred) else { return };
    let mut argv = mcv("admin user add vtminio mesh-cache-mbp");
    argv.push(secret.clone());
    push(report, "add", argv, Some(&secret));
}
RS
git add -A >/dev/null
gate
[ "$code" = 10 ] || { echo "    the I2674 shape was not refused with 10 ($code): $out"; exit 1; }
printf '%s\n' "$out" | grep -q '^apps/x/src/provision.rs:4: secret reaches the command line of argv.push (rust)$' \
  || { echo "    the finding does not name the file, the line and the sink: $out"; exit 1; }
printf '%s\n' "$out" | grep -q 'rule    project.a-secret-never-travels-on-a-command-line' \
  || { echo "    the finding does not name the rule: $out"; exit 1; }
echo "    a secret read from a credentials file and pushed onto an argv is refused at its line"

# ---------------------------------------------------------------- 2. the fixed form
cat > apps/x/src/provision.rs <<'RS'
fn identity(cred: &Path, report: &mut Report) {
    let Some(secret) = secret_of(cred) else { return };
    // the alias and the access key alone; mc reads the secret from its standard input
    let argv = mcv("admin user add vtminio mesh-cache-mbp");
    push(report, "add", argv, Some(&secret));
}
RS
gate
[ "$code" = 0 ] || { echo "    the secret on standard input was refused ($code): $out"; exit 1; }
echo "    the same secret as the child's standard input passes"

# ---------------------------------------------------------------- 3. shell
cat > scripts/publish <<SH
#!/bin/sh
api_token='$PLACEHOLDER'
curl -fsS -H "Authorization: Bearer \$api_token" https://api.invalid/user
SH
cat >> scripts/publish <<'SH'
key=$(security find-generic-password -s mesh -w)
mc alias set store http://127.0.0.1:9000 mesh-cache-mbp "$key"
SH
chmod +x scripts/publish
git add -A >/dev/null
gate --format json
[ "$code" = 10 ] || { echo "    the shell arguments were not refused with 10 ($code): $out"; exit 1; }
printf '%s' "$out" | jq -e '[.findings[] | "\(.path):\(.line):\(.sink):\(.value)"] == ["scripts/publish:3:curl:$api_token","scripts/publish:5:mc:$key"]' >/dev/null \
  || { echo "    the shell findings are not the curl and the mc lines: $out"; exit 1; }
printf '%s' "$out" | jq -e '.measured and (.passes | not) and .rule == "project.a-secret-never-travels-on-a-command-line" and .shell_files >= 1' >/dev/null \
  || { echo    "    the report's verdict is wrong: $out"; exit 1; }
case "$out" in *"$PLACEHOLDER"*) echo "    the report carries the value it found"; exit 1 ;; esac
echo "    a token and a keychain secret as arguments of curl and mc are refused; the value is never repeated"

# ---------------------------------------------------------------- 4. over MCP
mcp="$( { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3996","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_quality_secret_argv","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo "$PWD" 2>"$S/mcp.err" | sed -n 2p | jq -c '.result.structuredContent // empty')"
[ "$(printf '%s' "$mcp" | jq -c '[.findings[] | [.path, .line, .sink]]')" = "$(printf '%s' "$out" | jq -c '[.findings[] | [.path, .line, .sink]]')" ] \
  || { echo "    MCP and the command line disagree: $mcp"; cat "$S/mcp.err"; exit 1; }
echo "    a tools/call over MCP answers the same findings"

# ---------------------------------------------------------------- 5. the forms that pass
cat > scripts/publish <<SH
#!/bin/sh
api_token='$PLACEHOLDER'
printf '%s\n' "\$api_token" | mc admin user add store mesh-cache-mbp
TOKEN="\$api_token" gh api /user
mc admin user add store mesh-cache-mbp <<<"\$api_token"
# secret-argv: a fixture's public key, not a credential
ssh-keygen -l -f /dev/stdin "\$api_token"
SH
gate
[ "$code" = 0 ] || { echo    "    standard input, the environment or a declared line was refused ($code): $out"; exit 1; }
echo "    the token on standard input, in the environment, or on a line that says why, passes"

# ---------------------------------------------------------------- 6. this repository
gate --repo "$ROOT"
[ "$code" = 0 ] || { echo "    this repository hands a secret to a command line ($code): $out"; exit 1; }
echo "    this repository passes: $(printf '%s\n' "$out" | tail -n 1)"
