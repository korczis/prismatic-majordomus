# majordomus-covers: none
# Every published tree draws its diagrams (I2746, project.a-diagram-is-drawn-not-typed@2).
#
# Version 1 of the rule read docs/, site/content-src/ and site/templates/ only, so box art in
# a decision, a rule, the handbook or the markdown `majordomus ui` renders was invisible to
# the gate. This case plants a drawing in each tree version 2 added and requires
# scripts/ci/diagram-check to name every one:
#
#   caught      box art in an ADR, arrows in a rule, a <pre> in a handbook chapter, a fence
#               in a UI module's doc comment, a fence in a UI string, a <pre> in a UI script,
#               a `▼`-only drawing and a drawing whose left margin looks like a tree
#   not caught  a mermaid block in an ADR, a file listing in a rule, a handbook
#               `<pre class="mermaid-src" data-lang="mermaid">`, a `console` transcript in a
#               UI doc comment, a campaign pack (verbatim provenance, never published) and
#               site/content (a projection nobody edits)
#
# and the real tree holds no hand-drawn diagram at all.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/diagram-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
command -v python3 >/dev/null 2>&1 || skip "python3 absent"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3106.XXXXXX")"
trap 'rm -rf "$S"' EXIT

fixture() {  # fixture <dir>
  mkdir -p "$1/.ai/repo/adrs" "$1/.ai/repo/rules/project" "$1/handbook/cs/sources" \
    "$1/apps/majordomus-cli/src/ui" "$1/share/ui" "$1/docs" "$1/campaigns/pack" \
    "$1/site/content/adrs"
  printf 'diagrams=0\n' > "$1/.ai/repo/diagram-baseline.txt"
}

counted() {  # counted <fixture-dir>: the gate's own count
  MJ_ROOT="$1" "$GATE" --update >/dev/null 2>&1
  sed -n 's/^diagrams=\([0-9]*\)$/\1/p' "$1/.ai/repo/diagram-baseline.txt"
}

# ------------------------------------------------------------- planted in every new tree
P="$S/planted"; fixture "$P"
cat > "$P/.ai/repo/adrs/0001-a-decision.md" <<'MD'
# 1. A decision

```text
capability!  ──►  CapabilityRegistry  ──►  Context::execute
                          │
                          └──►  ExecutionEngine
```
MD
cat > "$P/.ai/repo/rules/project/a-rule.v1.md" <<'MD'
# Required behaviour

```
intent  -->  plan  -->  execute
```
MD
cat > "$P/handbook/cs/sources/chapter.html" <<'HTML'
<section>
  <div class="code"><pre><code>principle → doctrine → validator → claim</code></pre></div>
</section>
HTML
cat > "$P/apps/majordomus-cli/src/ui/screen.rs" <<'RS'
//! A screen.
//!
//! ```text
//!   capability! ── registry ── executor ──┬── MCP, HTTP, command line
//!                                         ├── UI       /ui
//!                                         └── Cockpit  /cockpit
//! ```
pub fn screen() {}
RS
cat > "$P/apps/majordomus-cli/src/ui/help.rs" <<'RS'
/// The help text a screen draws.
pub const HELP: &str = r#"
How a request travels:

```
request ──▶ router ──▶ screen
```
"#;
RS
cat > "$P/share/ui/help.js" <<'JS'
// The overlay's markup.
const HELP = '<pre><code>request ──▶ router ──▶ screen</code></pre>';
JS
# a drawing whose only arrows point down, and one whose left margin reads like a listing
cat > "$P/docs/DOWN.md" <<'MD'
```text
ONE DECLARATION
        ↓
REGISTRY
        ▼
ENGINE
```
MD
cat > "$P/docs/MARGIN.md" <<'MD'
```text
executor ──┬── MCP
           ├── UI
           └── Cockpit
```
MD
found="$(counted "$P")"
[ "$found" = 8 ] || {
  echo "    the gate found $found of 8 drawings planted in the trees version 2 covers:"
  MJ_ROOT="$P" "$GATE" --list; exit 1; }
printf 'diagrams=0\n' > "$P/.ai/repo/diagram-baseline.txt"
expect_exit 10 env MJ_ROOT="$P" "$GATE"
expect_grep 'rose from 0 to 8'
expect_grep '\.ai/repo/adrs/0001-a-decision\.md'
expect_grep '\.ai/repo/rules/project/a-rule\.v1\.md'
expect_grep 'handbook/cs/sources/chapter\.html'
expect_grep 'apps/majordomus-cli/src/ui/screen\.rs'
expect_grep 'share/ui/help\.js'
expect_grep 'docs/DOWN\.md'
expect_grep 'docs/MARGIN\.md'
expect_grep 'apps/majordomus-cli/src/ui/help\.rs'

# ------------------------------------------------------------- what stays clean there
C="$S/clean"; fixture "$C"
cat > "$C/.ai/repo/adrs/0001-a-decision.md" <<'MD'
# 1. A decision

```mermaid
flowchart LR
  registry["CapabilityRegistry"] --> execute["Context::execute"]
```
MD
cat > "$C/.ai/repo/rules/project/a-rule.v1.md" <<'MD'
```text
.ai/repo/rules/
├── vendor/majordomus/         the pinned baseline
│   └── rules/*.md
└── project/*.md               rules this repository wrote
```
MD
cat > "$C/handbook/cs/sources/chapter.html" <<'HTML'
<pre class="mermaid-src" data-lang="mermaid">flowchart LR
  principle["principle"] --&gt; doctrine["doctrine"] --&gt; claim["claim"]</pre>
HTML
cat > "$C/apps/majordomus-cli/src/ui/screen.rs" <<'RS'
//! A screen.
//!
//! ```console
//! $ majordomus ui
//!   ──────────────
//!   → http://127.0.0.1:8741/ui
//! ```
pub fn screen() {}
RS
# a campaign pack is verbatim provenance, and site/content is a projection: neither is read
cat > "$C/campaigns/pack/README.md" <<'MD'
```
official source ────────────────┐
                                ▼
                            the pack
```
MD
cat > "$C/site/content/adrs/adr-0001.md" <<'MD'
```text
capability!  ──►  CapabilityRegistry
```
MD
found="$(counted "$C")"
[ "$found" = 0 ] || {
  echo "    the gate caught $found block(s) that are not drawings, or are not published:"
  MJ_ROOT="$C" "$GATE" --list; exit 1; }
expect_exit 0 env MJ_ROOT="$C" "$GATE"
expect_grep 'no hand-drawn diagram'

# ------------------------------------------------------------- this repository draws them all
expect_exit 0 "$GATE"
expect_grep 'no hand-drawn diagram'
grep -q '^diagrams=0$' "$ROOT/.ai/repo/diagram-baseline.txt" || {
  echo "    the baseline is not zero: every drawing in a covered tree is to be converted"; exit 1; }
