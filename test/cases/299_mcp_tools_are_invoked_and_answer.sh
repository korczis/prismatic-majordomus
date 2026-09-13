# majordomus-covers: none
# Ten MCP tools nothing had ever asked a question of, asked one — and the ratchet that stops
# the eleventh from arriving unasked.
#
# The registry projects 109 tools. `use_cases.coverage.mcp_tools` is advisory, so a tool no
# use case names is one line of INFO in `doctor` and fails nothing; and for this class
# `covered` means NAMED, because a use case's scenario runs `bin/majordomus <argv>` and can
# never reach an MCP tool at all (lib/usecase.sh, mj_uc_coverage_rows). A use case listing
# five tools in its front matter and running `init` and `doctor` marks all five covered.
#
# apps/majordomus-cli/tests/properties.rs does send every capability a real tools/call frame,
# and asserts the MCP answer equals the HTTP answer equals the direct answer. That is an
# equivalence over a SyntheticRepository of twelve rules and four documents: three surfaces
# agreeing on an empty answer is three surfaces agreeing. This case asks the other question —
# does the tool answer CORRECTLY about a real repository — of a sample of ten, and every
# assertion below is a relation between what the tool said and what this checkout holds,
# never a number written down here. A literal count would rot the day someone adds an issue.
#
# The repository it asks about is the checkout the suite lives in, read-only: `--standalone`
# binds no port, writes no lease and writes nothing anywhere, and every sampled capability
# declares `effect: read` but one, which is asked to refuse. test/run.sh permits a case to
# read its checkout; it may not write into it, and this one does not.
#
# The second half is the gate: scripts/ci/mcp-tool-run-check, the ratchet that holds the
# uninvoked set flat. It is proved by mutation, over synthetic trees built here, and every
# mutation is asserted to have been applied to the tree before its verdict is read — a patch
# that silently no-ops turns the suite into the instrument that lies.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj299.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    jq is required by this case"; exit 1; }
GATE="$ROOT/scripts/ci/mcp-tool-run-check"

# One MCP session per call, against this checkout, alone: no port, no lease, nothing written.
# MAJORDOMUS_SHARE is unset for the child because an exported one from the operator's shell
# points at another checkout's distribution and makes the answer that checkout's.
call() { # tool arguments -> the JSON-RPC response frame on stdout
  { printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case299","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | env -u MAJORDOMUS_SHARE "$RB" mcp --standalone --repo "$ROOT" 2>"$S/err" | sed -n 2p
}
answer() { # tool arguments file: the typed answer, or the case fails naming the tool
  call "$1" "$2" > "$S/frame.json"
  jq -e '.result.structuredContent' "$S/frame.json" > "$3" 2>/dev/null \
    || { echo "    $1 returned no typed answer:"; head -c 600 "$S/frame.json"; echo; sed 's/^/    | /' "$S/err" | head -5; exit 1; }
}
bad() { echo "    $1"; shift; [ $# -eq 0 ] || jq -c . "$1" | head -c 600; echo; exit 1; }

# ---------------------------------------------------------------- 1. artifacts.list
# The manifest it reports is the manifest in the tree, document for document.
answer majordomus_artifacts '{}' "$S/artifacts.json"
want="$(jq '.documents | length' "$ROOT/docs/generated/artifacts.json")"
jq -e --argjson n "$want" '.present == true and .manifest == "docs/generated/artifacts.json" and .tallies.documents == $n' "$S/artifacts.json" >/dev/null \
  || bad "majordomus_artifacts does not report the $want document(s) docs/generated/artifacts.json holds:" "$S/artifacts.json"

# ---------------------------------------------------------------- 2. deploy.get
# The declared deployment, read back by its own id, carrying the file it came from.
answer majordomus_deployment '{"id":"majordomus"}' "$S/deployment.json"
want="$(awk '$1 == "application:" { print $2; exit }' "$ROOT/.ai/repo/deployments/majordomus.yaml")"
jq -e --arg a "$want" '.id == "majordomus" and .kind == "deployment" and .file == ".ai/repo/deployments/majordomus.yaml" and .application == $a' "$S/deployment.json" >/dev/null \
  || bad "majordomus_deployment does not answer with the declaration in .ai/repo/deployments/majordomus.yaml (application: $want):" "$S/deployment.json"

# ---------------------------------------------------------------- 3. devcontext.policy
# The tier ladder the context compiler selects by, ordered and each naming the kinds it holds.
answer majordomus_devcontext_policy '{}' "$S/devpolicy.json"
jq -e '(.tiers | length) >= 5 and .default_budget_tokens > 0 and ([.tiers[] | select((.tier | length) == 0 or (.kinds | length) == 0)] | length) == 0' "$S/devpolicy.json" >/dev/null \
  || bad "majordomus_devcontext_policy does not answer with a tier ladder that names its kinds:" "$S/devpolicy.json"
jq -e '[.tiers[].position] == ([.tiers[].position] | sort)' "$S/devpolicy.json" >/dev/null \
  || bad "the tiers are not in position order:" "$S/devpolicy.json"

# ---------------------------------------------------------------- 4. executions.cancel
# The one sampled capability that is not a read. It is asked to refuse, and a refusal is a
# result with isError, never a transport error — the crate's own contract.
call majordomus_execution_cancel '{"id":"not-an-execution"}' > "$S/cancel.json"
jq -e '.result.isError == true and (.error | not)' "$S/cancel.json" >/dev/null \
  || bad "majordomus_execution_cancel does not refuse an id that is not one as a result:" "$S/cancel.json"
jq -e '[.result.content[].text] | join(" ") | test("execution id")' "$S/cancel.json" >/dev/null \
  || bad "the refusal does not say what an execution id looks like:" "$S/cancel.json"

# ---------------------------------------------------------------- 5. distribution.status
# The version is the crate's, the stable channel resolves to a release this tree records,
# and the required targets are that record's own.
answer majordomus_install_status '{}' "$S/install.json"
want="$(awk -F'"' '/^version = "/ { print $2; exit }' "$ROOT/apps/majordomus-cli/Cargo.toml")"
jq -e --arg v "$want" '.local_version == $v' "$S/install.json" >/dev/null \
  || bad "majordomus_install_status does not report the crate's version ($want):" "$S/install.json"
tag="$(jq -r '.stable_tag' "$S/install.json")"
[ -f "$ROOT/.ai/repo/releases/$tag.yaml" ] \
  || { echo "    the stable channel resolves to $tag, which .ai/repo/releases does not record"; exit 1; }
want="$(awk '/^required_targets:/ { n = 1; next } n && /^  - / { c++; next } n && /^[^ ]/ { exit } END { print c + 0 }' "$ROOT/.ai/repo/releases/$tag.yaml")"
jq -e --argjson n "$want" '.required_targets == $n' "$S/install.json" >/dev/null \
  || bad "the required targets are not the $want that .ai/repo/releases/$tag.yaml records:" "$S/install.json"
# no network is reached from here, and the report says so rather than guessing
jq -e '[.checks[] | select(.id == "public")] | length == 1 and (.[0].state == "unknown")' "$S/install.json" >/dev/null \
  || bad "the published half is not reported as unchecked, so the report is claiming what it cannot see:" "$S/install.json"

# ---------------------------------------------------------------- 6. mesh.identity
# Machine state, not repository state: the answer is where it would be and whether it is
# there. Both outcomes are correct; an answer that does not say which is not.
answer majordomus_mesh_identity '{}' "$S/mesh.json"
jq -e '(.path | test("node\\.json$")) and (.present | type) == "boolean"' "$S/mesh.json" >/dev/null \
  || bad "majordomus_mesh_identity does not say where the node identity is and whether it exists:" "$S/mesh.json"

# ---------------------------------------------------------------- 7. plan.issues
# Every issue the project store holds, and a narrowing that actually narrows.
answer majordomus_plan_issues '{}' "$S/issues.json"
want="$(find "$ROOT/.ai/repo/project/issues" -name 'I*.yaml' | wc -l | tr -d ' ')"
jq -e --argjson n "$want" '.total == $n and (.issues | length) == $n' "$S/issues.json" >/dev/null \
  || bad "majordomus_plan_issues does not answer with the $want issue(s) .ai/repo/project/issues holds:" "$S/issues.json"
answer majordomus_plan_issues '{"status":"DONE"}' "$S/issues-done.json"
jq -e '(.issues | length) == .total and .total > 0 and ([.issues[] | select(.status != "DONE")] | length) == 0' "$S/issues-done.json" >/dev/null \
  || bad "the status filter does not narrow to DONE:" "$S/issues-done.json"
jq -e --slurpfile all "$S/issues.json" '.total < $all[0].total' "$S/issues-done.json" >/dev/null \
  || bad "the filtered answer is not smaller than the whole, so the filter did nothing:" "$S/issues-done.json"

# ---------------------------------------------------------------- 8. product.validate
# The product model of this repository, judged by the tool that owns it.
answer majordomus_product_validate '{}' "$S/product.json"
jq -e '.valid == true and .errors == 0 and .counts.features > 0 and .counts.commands > 0' "$S/product.json" >/dev/null \
  || bad "majordomus_product_validate reports this repository's product model as invalid; read the findings before changing this case:" "$S/product.json"

# ---------------------------------------------------------------- 9. rules.report
# The head it answers about is this checkout's head, the population is the rules section on
# disk, and the class filter partitions it rather than filtering nothing.
answer majordomus_rules '{}' "$S/rules.json"
want="$(git -C "$ROOT" rev-parse HEAD)"
jq -e --arg h "$want" '.head == $h' "$S/rules.json" >/dev/null \
  || bad "majordomus_rules answers about $(jq -r .head "$S/rules.json"), not about this checkout's head $want:" "$S/rules.json"
want="$(find "$ROOT/.ai/repo/rules" -name '*.md' ! -name 'README.md' | wc -l | tr -d ' ')"
jq -e --argjson n "$want" '.coverage.rules == $n' "$S/rules.json" >/dev/null \
  || bad "majordomus_rules does not measure the $want rule(s) .ai/repo/rules holds:" "$S/rules.json"
answer majordomus_rules '{"class":"blocking"}' "$S/rules-blocking.json"
answer majordomus_rules '{"class":"advisory"}' "$S/rules-advisory.json"
b="$(jq '.rules | length' "$S/rules-blocking.json")"; a="$(jq '.rules | length' "$S/rules-advisory.json")"
jq -e --argjson b "$b" --argjson a "$a" '.coverage.rules == ($b + $a) and .coverage.blocking == $b and .coverage.advisory == $a' "$S/rules.json" >/dev/null \
  || { echo "    the class filter does not partition the rules: $b blocking + $a advisory against"; jq -c '.coverage' "$S/rules.json"; exit 1; }

# ---------------------------------------------------------------- 10. web.surfaces
# What this process serves and what is published, each surface saying which it is.
answer majordomus_web_surfaces '{}' "$S/web.json"
jq -e '(.surfaces | length) > 0 and (.findings | type) == "array" and (.public | index("api")) != null' "$S/web.json" >/dev/null \
  || bad "majordomus_web_surfaces does not report the API among the public surfaces:" "$S/web.json"
jq -e '([.surfaces[] | select((.id | length) == 0 or (.category | length) == 0 or (.availability | length) == 0)] | length) == 0' "$S/web.json" >/dev/null \
  || bad "a surface does not say what it is or where it is available:" "$S/web.json"
jq -e '[.surfaces[].id] as $ids | ((.served + .published) | unique) - $ids | length == 0' "$S/web.json" >/dev/null \
  || bad "a surface is listed as served or published without being one of the surfaces:" "$S/web.json"

echo "    ten MCP tools answered about this repository, and every answer matched the tree"

# ================================================================ the ratchet, by mutation
# A synthetic tree the gate can be pointed at: a registry projecting three tools, one client
# that invokes one of them, and a baseline holding the other two. Everything below mutates
# this tree, asserts the mutation is in the tree, and only then reads the verdict.
G="$S/gate"; mkdir -p "$G/docs/generated" "$G/test/cases" "$G/.ai/repo"
registry() { # tool... -> a registry projecting exactly these tools
  { printf '{"schema":"x","capabilities":['
    local first=1 t
    for t in "$@"; do
      [ "$first" = 1 ] || printf ','
      first=0
      printf '{"id":"%s","exposure":{"mcp":{"tool": "%s"}}}' "${t#majordomus_}" "$t"
    done
    printf ']}\n'
  } > "$G/docs/generated/registry.json"
}
client() { cat > "$G/test/cases/10_client.sh"; }
baseline() { { echo "# written by the case"; [ $# -eq 0 ] || printf '%s\n' "$@"; } > "$G/.ai/repo/mcp-tool-run-baseline.txt"; }
gate() { # -> exit code in $rc, output in $S/gate.log
  rc=0
  env MJ_ROOT="$G" bash "$GATE" > "$S/gate.log" 2>&1 || rc=$?
}
registry majordomus_alpha majordomus_beta majordomus_gamma
client <<'CLIENT'
# a client of the synthetic tree
frame() { printf '{"method":"tools/call","params":{"name":"%s"}}' "$1"; }
frame majordomus_alpha
CLIENT
baseline majordomus_beta majordomus_gamma

# --- M0: the tree as built. The mutation is asserted before the verdict is read.
grep -q '^frame majordomus_alpha$' "$G/test/cases/10_client.sh" || { echo "    M0 not applied: the client does not invoke alpha"; exit 1; }
grep -q '^majordomus_beta$' "$G/.ai/repo/mcp-tool-run-baseline.txt" || { echo "    M0 not applied: beta is not in the baseline"; exit 1; }
gate
[ "$rc" = 0 ] || { echo "    M0: the recorded debt should pass, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep '3 projected by' "$S/gate.log"
expect_grep '2 tool\(s\) known, 1 of 3 invoked' "$S/gate.log"

# --- M1: THE RISE. A tool arrives that nothing invokes and the baseline does not excuse.
# This is the mutation the whole gate exists for; if it does not fail here, the gate is
# decoration and the INFO it replaced was worth as much.
registry majordomus_alpha majordomus_beta majordomus_gamma majordomus_delta
grep -q '"tool": "majordomus_delta"' "$G/docs/generated/registry.json" || { echo "    M1 not applied: delta is not in the registry"; exit 1; }
if grep -q '^majordomus_delta$' "$G/.ai/repo/mcp-tool-run-baseline.txt"; then echo "    M1 not applied: delta is already excused by the baseline"; exit 1; fi
gate
[ "$rc" = 10 ] || { echo "    M1: a new uninvoked tool must fail the gate, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'new debt' "$S/gate.log"
expect_grep 'majordomus_delta' "$S/gate.log"
# and it must fail for delta alone: the two the baseline holds are KNOWN, not FAIL
expect_grep 'KNOWN mcp-tool-run uninvoked majordomus_beta' "$S/gate.log"
expect_no_grep 'FAIL mcp-tool-run uninvoked majordomus_beta' "$S/gate.log"

# --- M2: the debt is paid the only way it may be — by invoking the tool.
client <<'CLIENT'
# a client of the synthetic tree
frame() { printf '{"method":"tools/call","params":{"name":"%s"}}' "$1"; }
frame majordomus_alpha
frame majordomus_beta
frame majordomus_delta
CLIENT
grep -q '^frame majordomus_beta$' "$G/test/cases/10_client.sh" || { echo "    M2 not applied: the client does not invoke beta"; exit 1; }
grep -q '^frame majordomus_delta$' "$G/test/cases/10_client.sh" || { echo "    M2 not applied: the client does not invoke delta"; exit 1; }
gate
[ "$rc" = 0 ] || { echo "    M2: invoking the new tool must clear the debt, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'debt cleared' "$S/gate.log"
expect_grep 'majordomus_beta' "$S/gate.log"

# --- M3: a tool named in a COMMENT is discussed, not invoked. The detector that counts a
# comment is the one that marks a surface covered by a run that does not touch it.
baseline majordomus_beta
client <<'CLIENT'
# a client of the synthetic tree
# majordomus_gamma is named here in prose and never invoked
frame() { printf '{"method":"tools/call","params":{"name":"%s"}}' "$1"; }
frame majordomus_alpha
frame majordomus_beta
frame majordomus_delta
CLIENT
grep -q '^# majordomus_gamma is named here' "$G/test/cases/10_client.sh" || { echo "    M3 not applied: the comment naming gamma is not in the client"; exit 1; }
if grep -q '^frame majordomus_gamma$' "$G/test/cases/10_client.sh"; then echo "    M3 not applied: gamma is invoked for real"; exit 1; fi
if grep -q '^majordomus_gamma$' "$G/.ai/repo/mcp-tool-run-baseline.txt"; then echo "    M3 not applied: gamma is still excused by the baseline"; exit 1; fi
gate
[ "$rc" = 10 ] || { echo "    M3: a tool named only in a comment must stay uninvoked, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'FAIL mcp-tool-run uninvoked majordomus_gamma' "$S/gate.log"

# --- M4: a file that sends no tools/call is not a client, whatever it names.
cat > "$G/test/cases/11_not_a_client.sh" <<'NOTCLIENT'
echo majordomus_gamma
NOTCLIENT
grep -q '^echo majordomus_gamma$' "$G/test/cases/11_not_a_client.sh" || { echo "    M4 not applied: the non-client does not name gamma"; exit 1; }
if grep -q 'tools/call' "$G/test/cases/11_not_a_client.sh"; then echo "    M4 not applied: the non-client sends a tools/call"; exit 1; fi
gate
[ "$rc" = 10 ] || { echo "    M4: naming a tool outside a client must not count as invoking it, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'FAIL mcp-tool-run uninvoked majordomus_gamma' "$S/gate.log"
rm -f "$G/test/cases/11_not_a_client.sh"

# --- M5: a population that vanished is not a population that came back clean.
registry
if grep -q '"tool"' "$G/docs/generated/registry.json"; then echo "    M5 not applied: the registry still projects a tool"; exit 1; fi
gate
[ "$rc" = 12 ] || { echo "    M5: an empty registry must be unusable, not clean, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'not the same as nothing being wrong' "$S/gate.log"

# --- M6: no registry at all is unusable too, and says what to run.
rm -f "$G/docs/generated/registry.json"
if [ -e "$G/docs/generated/registry.json" ]; then echo "    M6 not applied: the registry is still there"; exit 1; fi
gate
[ "$rc" = 12 ] || { echo "    M6: a tree with no registry must be unusable, exit $rc"; cat "$S/gate.log"; exit 1; }
expect_grep 'majordomus generate' "$S/gate.log"

# --- and this repository's own verdict: the recorded debt passes, and the ten tools above
# are no longer in it. The baseline is the measured number, so this asserts the relation
# rather than a count written here.
rc=0; bash "$GATE" > "$S/real.log" 2>&1 || rc=$?
[ "$rc" = 0 ] || { echo "    the gate does not pass on this repository, exit $rc"; cat "$S/real.log"; exit 1; }
for tool in majordomus_artifacts majordomus_deployment majordomus_devcontext_policy \
            majordomus_execution_cancel majordomus_install_status majordomus_mesh_identity \
            majordomus_plan_issues majordomus_product_validate majordomus_rules \
            majordomus_web_surfaces; do
  if grep -qE "^[[:space:]]*${tool}[[:space:]]*$" "$ROOT/.ai/repo/mcp-tool-run-baseline.txt"; then
    echo "    $tool is invoked by this case and must not be in the baseline"; exit 1
  fi
done
echo "    the ratchet fails on a rise, clears on an invocation, and refuses an empty population"
