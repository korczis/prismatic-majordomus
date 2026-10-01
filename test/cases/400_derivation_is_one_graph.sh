# majordomus-covers: none
# claims: derivation-one-graph, landing-page-is-a-projection
# majordomus-timeout: 300
# The derivation is one graph, run by one command and checked by one read-only command.
#
# The real derivation takes minutes over the whole tree, so the first half of this case runs
# the repository's own scripts/derive and scripts/derive-check, unmodified and copied into a
# fixture, over stand-in generators that form a small graph with the same shape: stage A
# (`majordomus generate`) writes a.txt from the source, stage B (scripts/generate-site-data)
# writes b.txt from a.txt, stage C (`majordomus generate` again) writes c.txt from b.txt, and
# stage D (scripts/gitattributes) writes the merge policy over what the stages wrote. Every
# stand-in has a --check that names the files it would change. What is asserted is what the
# two scripts own: the order, that a second derivation on a clean tree changes nothing, that
# the check writes nothing and names every stale artifact from every generator in one report,
# and that a refusal (exit 15) is carried out as itself and stops the graph. That each real
# generator is idempotent and embeds no commit is 51_derived_artifacts_committed's.
#
# The second half is the homepage as a projection: a copy of this checkout's tracked files,
# where scripts/site-check must name a feature route, a provider file and a number written by
# hand, and `majordomus generate --check` must refuse a product dataset that does not match
# the repository. It needs the executable and jq, and skips that half without them.
. "$ROOT/test/lib.sh"

# ------------------------------------------------------------------ the graph, in a fixture
G="$T/graph"; TOOL="$T/tool"; LOG="$T/calls.log"
mkdir -p "$G/scripts" "$G/lib" "$G/src" "$G/out" "$TOOL"
cp "$ROOT/scripts/derive" "$ROOT/scripts/derive-check" "$G/scripts/"
cp "$ROOT/lib/rust_bin.sh" "$G/lib/"
printf 'v1\n' > "$G/src/model.txt"

# stage A and C: majordomus generate [--check] --repo R --strict
cat > "$TOOL/majordomus" <<'SH'
#!/usr/bin/env bash
set -eu
check=0; repo=""
while [ $# -gt 0 ]; do case "$1" in
  --check) check=1; shift ;; --repo) repo="$2"; shift 2 ;; *) shift ;; esac; done
printf 'generate%s\n' "$([ "$check" = 1 ] && echo ' --check')" >> "$STUB_LOG"
[ -z "${STUB_REFUSE:-}" ] || { echo "majordomus: not of this tree's generation" >&2; exit 15; }
cd "$repo"
stale=""
emit() { # emit PATH CONTENT
  if [ "$check" = 1 ]; then
    [ "$(cat "$1" 2>/dev/null)" = "$2" ] || stale="$stale $1 (differs)"
  else printf '%s\n' "$2" > "$1"; fi
}
emit out/a.txt "a($(cat src/model.txt))"
[ -f out/b.txt ] && emit out/c.txt "c($(cat out/b.txt))"
[ -z "$stale" ] || { echo "majordomus: generated artifact(s) stale:$stale"; exit 10; }
SH
# stage B: scripts/generate-site-data [--check]
cat > "$G/scripts/generate-site-data" <<'SH'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
printf 'site-data%s\n' "${1:+ $1}" >> "$STUB_LOG"
want="b($(cat out/a.txt))"
if [ "${1:-}" = --check ]; then
  [ "$(cat out/b.txt 2>/dev/null)" = "$want" ] || { echo "generate-site-data: stale: out/b.txt"; exit 10; }
else printf '%s\n' "$want" > out/b.txt; fi
SH
# stage D: scripts/gitattributes [--check]
cat > "$G/scripts/gitattributes" <<'SH'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
printf 'gitattributes%s\n' "${1:+ $1}" >> "$STUB_LOG"
want="$(for f in out/*.txt; do printf '%s merge=derived\n' "$f"; done)"
if [ "${1:-}" = --check ]; then
  [ "$(cat .gitattributes 2>/dev/null)" = "$want" ] || { echo "gitattributes: stale: .gitattributes"; exit 10; }
else printf '%s\n' "$want" > .gitattributes; fi
SH
chmod +x "$TOOL/majordomus" "$G/scripts/generate-site-data" "$G/scripts/gitattributes"
STUB_LOG="$LOG"; export STUB_LOG
( cd "$G" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm sources )

derive()       { MAJORDOMUS_BIN="$TOOL/majordomus" bash "$G/scripts/derive"; }
derive_check() { MAJORDOMUS_BIN="$TOOL/majordomus" bash "$G/scripts/derive-check"; }
tree_state()   { ( cd "$G" && git status --porcelain --untracked-files=all && git diff && cat .gitattributes out/*.txt 2>/dev/null ) || true; }

# --- one command regenerates every artifact, in dependency order
: > "$LOG"
expect_exit 0 derive
want='generate
site-data
generate
gitattributes'
[ "$(cat "$LOG")" = "$want" ] \
  || { printf '    scripts/derive did not run the stages in dependency order\n    want:\n%s\n    got:\n%s\n' "$want" "$(cat "$LOG")"; exit 1; }
[ "$(cat "$G/out/c.txt")" = "c(b(a(v1)))" ] \
  || { echo "    one derivation did not carry the source through every stage: c.txt is '$(cat "$G/out/c.txt")'"; exit 1; }
( cd "$G" && git add -A && git commit -qm derived )

# --- the check passes on the derived tree, and writes nothing
before="$(tree_state)"
expect_exit 0 derive_check
expect_grep 'every derived artifact is current'
[ "$(tree_state)" = "$before" ] || { echo "    scripts/derive-check changed the tree on a current one"; exit 1; }

# --- a second derivation on a clean tree changes nothing
expect_exit 0 derive
[ -z "$(cd "$G" && git status --porcelain --untracked-files=all)" ] \
  || { echo "    a second derivation on a clean tree changed: $(cd "$G" && git status --porcelain)"; exit 1; }

# --- a stale tree: the check runs every generator's check before exiting, names every stale
#     artifact from each of them in one report, exits 10, and still writes nothing
printf 'v2\n' > "$G/src/model.txt"
printf 'tampered\n' > "$G/out/b.txt"
printf 'out/gone.txt merge=derived\n' >> "$G/.gitattributes"
before="$(tree_state)"
: > "$LOG"
expect_exit 10 derive_check
expect_grep 'out/a\.txt \(differs\)'
expect_grep 'out/c\.txt \(differs\)'
expect_grep 'stale: out/b\.txt'
expect_grep 'stale: \.gitattributes'
expect_grep 'run scripts/derive'
want='generate --check
site-data --check
gitattributes --check'
[ "$(cat "$LOG")" = "$want" ] \
  || { printf '    scripts/derive-check did not run every check in the order derive runs the generators\n    want:\n%s\n    got:\n%s\n' "$want" "$(cat "$LOG")"; exit 1; }
[ "$(tree_state)" = "$before" ] || { echo "    scripts/derive-check wrote to a stale tree"; exit 1; }

# --- and one derivation brings the whole chain back, which only dependency order can do
expect_exit 0 derive
[ "$(cat "$G/out/c.txt")" = "c(b(a(v2)))" ] \
  || { echo "    after a source change one derivation left c.txt at '$(cat "$G/out/c.txt")'"; exit 1; }
expect_exit 0 derive_check

# --- a refusal is the executable's verdict about itself: carried out as 15, nothing after it
( cd "$G" && git add -A && git commit -qm rederived )
: > "$LOG"
STUB_REFUSE=1 expect_exit 15 derive
expect_grep 'refused before anything was written'
[ "$(cat "$LOG")" = "generate" ] || { echo "    scripts/derive ran on past a refusal: $(tr '\n' ' ' < "$LOG")"; exit 1; }
: > "$LOG"
STUB_REFUSE=1 expect_exit 15 derive_check
expect_grep 'the tree is not stale'
[ "$(cat "$LOG")" = "generate --check" ] || { echo "    scripts/derive-check judged the site after a refusal: $(tr '\n' ' ' < "$LOG")"; exit 1; }

# --- a person reaches the same two scripts
JF="$(just_declaration)"
expect_grep '^    scripts/derive$' "$JF"
expect_grep '^    scripts/derive-check$' "$JF"

# ------------------------------------------------------------------ the homepage is a projection
command -v jq >/dev/null 2>&1 || skip "jq not installed (the homepage half)"
RB="$(rust_bin)" || rust_bin_exit $?
C="$T/copy"; mkdir -p "$C"; C="$(cd "$C" && pwd)"   # site-check names paths from its own `pwd`
# the tracked files as they are in this working tree, so the scripts under test are this
# tree's; a tracked file deleted in the working tree is left out
( cd "$ROOT" && git ls-files -z ) | while IFS= read -r -d '' f; do
  [ -f "$ROOT/$f" ] && printf '%s\0' "$f"
  :
done | ( cd "$ROOT" && tar --null -T - -cf - ) | ( cd "$C" && tar -xf - )
( cd "$C" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm copy )
PD="$C/site/data/registry/product.json"
expect_file "$PD"

# --- a stale product dataset fails the check the build runs: the executable refuses a tree whose
#     product.json does not match the repository, and names it. The copy is derived by the
#     executable first, so the baseline does not depend on whether this working tree was.
MAJORDOMUS_SHARE="$C/share" run_quiet "$T/gen.err" "$RB" generate --repo "$C" > /dev/null
( cd "$C" && git add -A && git commit -qm derived --allow-empty )
rc=0; MAJORDOMUS_SHARE="$C/share" "$RB" generate --repo "$C" --check --strict > "$T/gen0.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || { grep -v '^OK ' "$T/gen0.out" | tail -20; echo "    generate --check exited $rc on the copy it had just derived, not 0"; exit 1; }
fp0="$(bash "$C/scripts/generate-site-data" --fingerprint)"
cp "$PD" "$T/product.json.kept"
jq '.features[0].title = "A feature nobody declared"' "$PD" > "$PD.new" && mv "$PD.new" "$PD"
rc=0; MAJORDOMUS_SHARE="$C/share" "$RB" generate --repo "$C" --check --strict > "$T/gen1.out" 2>&1 || rc=$?
[ "$rc" = 10 ] || { tail -20 "$T/gen1.out"; echo "    generate --check exited $rc on a stale product dataset, not 10"; exit 1; }
expect_grep 'site/data/registry/product\.json \(differs\)' "$T/gen1.out"
# and the Pages build refuses it before rendering: its first step compares the site data's
# input hash, which covers product.json, with the committed one
fp1="$(bash "$C/scripts/generate-site-data" --fingerprint)"
[ -n "$fp0" ] && [ "$fp0" != "$fp1" ] \
  || { echo "    the site data's input fingerprint does not move with product.json; the Pages build would render a stale dataset"; exit 1; }
expect_grep 'MJ_PAGES_QUIET=1 timed fingerprint cmd_current \|\| return 10' "$ROOT/scripts/pages"
build_line="$(grep -n 'run: scripts/pages build' "$ROOT/.github/workflows/pages.yml" | head -1 | cut -d: -f1)"
publish_line="$(grep -n 'scripts/site-deploy --skip-build' "$ROOT/.github/workflows/pages.yml" | head -1 | cut -d: -f1)"
[ -n "$build_line" ] && [ -n "$publish_line" ] && [ "$build_line" -lt "$publish_line" ] \
  || { echo "    the Pages deploy job does not build (and so fingerprint) before it publishes"; exit 1; }
cp "$T/product.json.kept" "$PD"

# --- scripts/site-check names a feature route, a provider file and a number written by hand.
#     A minimal public tree: the other checks fail on it, and only the lines of these three
#     are asserted — each must name the one file it was planted in, and nothing else.
mkdir -p "$C/site/public/features/matrix"
printf '<html><body></body></html>\n' > "$C/site/public/index.html"
printf '<html></html>\n' > "$C/site/public/features/matrix/index.html"
route="$(jq -r '.features[0].route' "$PD")"
pfile="$(jq -r '[.providers[].bootstraps[]?.target, .providers[].client_config] | map(select(. != null)) | first' "$PD")"
[ -n "$route" ] && [ "$route" != null ] || { echo "    the product dataset declares no feature route"; exit 1; }
[ -n "$pfile" ] && [ "$pfile" != null ] || { echo "    the product dataset declares no provider file"; exit 1; }
printf '<a href="%s">by hand</a>\n' "$route" >> "$C/site/templates/features-section.html"
printf '<p>%s</p>\n' "$pfile" >> "$C/site/templates/partials/feature-surfaces.html"
printf 'handwritten_count = "42 commands"\n' >> "$C/site/data/marketing.toml"
rc=0; bash "$C/scripts/site-check" --no-sync > "$T/site-check.out" 2>&1 || rc=$?
[ "$rc" = 10 ] || { tail -20 "$T/site-check.out"; echo "    site-check exited $rc on a hand-named surface, not 10"; exit 1; }
hit="$(grep -F "FAIL product      the feature route $route is named by hand in:" "$T/site-check.out" | sed "s/ *$//" || true)"
[ "$hit" = "FAIL product      the feature route $route is named by hand in: $C/site/templates/features-section.html" ] \
  || { printf '    site-check did not name the hand-written feature route, and only where it was planted:\n%s\n' "$hit"; exit 1; }
hit="$(grep -F "FAIL product      the provider file $pfile is named by hand in:" "$T/site-check.out" | sed "s/ *$//" || true)"
[ "$hit" = "FAIL product      the provider file $pfile is named by hand in: $C/site/templates/partials/feature-surfaces.html" ] \
  || { printf '    site-check did not name the hand-written provider file, and only where it was planted:\n%s\n' "$hit"; exit 1; }
expect_grep '^FAIL promo +marketing\.toml contains a number' "$T/site-check.out"
# the counts the homepage prints are the dataset's: a page that prints them is not refused
for k in capabilities mcp_tools http_routes objects; do
  grep -qF "the homepage does not print the derived $k" "$T/site-check.out" \
    || { echo "    site-check did not refuse a homepage that prints no derived $k"; exit 1; }
done

# ------------------------------------------------------------------ no derived file names its commit
# The last clause of derivation-one-graph, on this repository's own derived files: an artifact
# that records the commit it lands in is stale the moment it lands, so a second regeneration
# could never leave the tree clean. Case 51 carries the same check for the committed tree; it is
# repeated here so the claim's one named test asserts the whole claim.
cur="$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || true)"
if [ -n "$cur" ]; then
  for f in "$ROOT"/site/data/generated/*.json "$ROOT/docs/SITE_CLAIMS.md" "$ROOT/docs/PLAN_STATUS.md"; do
    [ -f "$f" ] || continue
    if grep -qF -- "${cur:0:7}" "$f"; then
      printf '    %s embeds the current commit; it will be stale the moment it is committed\n' "${f#"$ROOT"/}"
      exit 1
    fi
  done
fi
