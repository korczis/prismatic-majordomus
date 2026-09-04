# Scoped context documents: discovery by contract, the ancestor chain, scope, composition,
# a deterministic order, provider and audience filtering, and every way the tree can be
# wrong — each refused by name, with the tree unchanged.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git add -A >/dev/null; git commit -qm install
A=.ai/repo/areas

# cdoc FILE ID SCOPE ORDER [FRONT-MATTER LINE ...]  — a minimal valid context document;
# a later line with the same key wins, so a scenario overrides a default by appending
cdoc() {
  local f="$1" id="$2" scope="$3" order="$4"; shift 4
  mkdir -p "$(dirname "$f")"
  { printf -- '---\nschema: context/v1\nid: %s\nkind: context\ntitle: Context %s\ndescription: What %s describes.\nstatus: active\nscope: %s\nproviders: ["*"]\ncomposition: extend\norder: %s\n' "$id" "$id" "$id" "$scope" "$order"
    for line in "$@"; do printf '%s\n' "$line"; done
    printf -- '---\n\n# %s\n' "$id"
  } > "$f"
}
# cset FILE KEY VALUE — replace one front-matter line in place
cset() { sed -i.bak "s|^$2:.*|$2: $3|" "$1"; rm -f "$1.bak"; }
chain() { "$MJ" context resolve "$1" | awk 'NR > 1 && $1 ~ /^[0-9]+$/ { print $2 }' | paste -sd' ' -; }

# ---------------------------------------------------------------- discovery
# the seeded layer already resolves: the root document, the tracked half, each section
expect_exit 0 "$MJ" context validate
expect_grep 'OK +context'
expect_exit 0 "$MJ" context list
expect_grep '^ai\.layer +\.ai/README\.md +subtree +final'
expect_grep '^ai\.repo\.rules +\.ai/repo/rules/README\.md'
[ "$(chain .ai/repo/rules/project)" = "ai.layer ai.repo ai.repo.rules" ] || { echo "    the seeded chain is wrong: $(chain .ai/repo/rules/project)"; exit 1; }

# a Markdown file with no front matter, and one of another kind, are not documents
printf '# notes\n\nno front matter here\n' > .ai/repo/notes.md
expect_exit 0 "$MJ" context validate
expect_exit 0 "$MJ" context list
expect_no_grep 'notes\.md'
"$MJ" context list | grep -q 'scope-integrity' && { echo "    a rule was listed as a context document"; exit 1; }
# a file the manifest names as a convention must carry the contract
mkdir -p "$A"; printf '# stray\n' > "$A/README.md"
expect_exit 10 "$MJ" context validate
expect_grep 'areas/README\.md.*invalid-front-matter.*manifest names README\.md'
rm -f "$A/README.md" .ai/repo/notes.md
# a document recognised by contract alone, whatever its name
cdoc "$A/guide.md" ai.repo.areas subtree 100
expect_exit 0 "$MJ" context validate
expect_exit 0 "$MJ" context list
expect_grep '^ai\.repo\.areas +\.ai/repo/areas/guide\.md'
# the local half is never a document
mkdir -p .ai/local/notes && cdoc .ai/local/notes/guide.md local.never subtree 1
expect_exit 0 "$MJ" context list
expect_no_grep 'local\.never'
rm -rf .ai/local/notes

# ---------------------------------------------------------------- inheritance and scope
cdoc "$A/alpha/guide.md"      ai.repo.areas.alpha      subtree   100
cdoc "$A/alpha/deep/guide.md" ai.repo.areas.alpha.deep subtree   100
cdoc "$A/beta/guide.md"       ai.repo.areas.beta       subtree   100
cdoc "$A/beta/here.md"        ai.repo.areas.beta.here  directory 100
mkdir -p "$A/beta/sub"
cdoc "$A/gamma/only.md"       ai.repo.areas.gamma      explicit  100 'paths: [.ai/repo/areas/alpha/deep]'
expect_exit 0 "$MJ" context validate
[ "$(chain $A)" = "ai.layer ai.repo ai.repo.areas" ] || { echo "    parent chain: $(chain $A)"; exit 1; }
[ "$(chain $A/alpha)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.alpha" ] || { echo "    child chain: $(chain $A/alpha)"; exit 1; }
[ "$(chain $A/alpha/deep)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.alpha ai.repo.areas.alpha.deep ai.repo.areas.gamma" ] \
  || { echo "    grandchild chain: $(chain $A/alpha/deep)"; exit 1; }
# a sibling never leaks; an explicit document applies only where it says
[ "$(chain $A/beta)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.beta ai.repo.areas.beta.here" ] || { echo "    sibling chain: $(chain $A/beta)"; exit 1; }
[ "$(chain $A/beta/sub)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.beta" ] || { echo "    directory scope leaked below: $(chain $A/beta/sub)"; exit 1; }
[ "$(chain $A/gamma)" = "ai.layer ai.repo ai.repo.areas" ] || { echo "    explicit applied to its own directory: $(chain $A/gamma)"; exit 1; }
# a file resolves to its directory; outside the tree the root chain applies, plus tracks
[ "$(chain $A/alpha/guide.md)" = "$(chain $A/alpha)" ] || { echo "    a file did not resolve to its directory"; exit 1; }
[ "$(chain .)" = "ai.layer" ] || { echo "    outside the tree: $(chain .)"; exit 1; }
mkdir -p lib && printf 'x\n' > lib/thing.sh && git add lib/thing.sh
cset "$A/beta/guide.md" providers '["*"]'; printf 'tracks: [lib/thing.sh]\n' >> "$A/beta/guide.md"
# the appended key must sit inside the front matter: move the closing fence below it
awk 'BEGIN { c = 0 } /^---$/ { c++; if (c == 2) { hold = 1; next } } hold && /^tracks:/ { print; print "---"; hold = 0; next } { print }' "$A/beta/guide.md" > "$A/beta/guide.tmp" && mv "$A/beta/guide.tmp" "$A/beta/guide.md"
expect_exit 0 "$MJ" context validate
[ "$(chain lib/thing.sh)" = "ai.layer ai.repo.areas.beta" ] || { echo "    tracks did not apply: $(chain lib/thing.sh)"; exit 1; }
expect_exit 0 "$MJ" context explain lib/thing.sh
expect_grep 'applies: tracks lib/thing\.sh'

# ---------------------------------------------------------------- ordering
# depth first; within a depth the declared order, then the path; the same twice
cdoc "$A/alpha/zz.md" ai.repo.areas.alpha.zz subtree 50
cdoc "$A/alpha/aa.md" ai.repo.areas.alpha.aa subtree 100
[ "$(chain $A/alpha)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.alpha.zz ai.repo.areas.alpha.aa ai.repo.areas.alpha" ] \
  || { echo "    order within a depth: $(chain $A/alpha)"; exit 1; }
"$MJ" context resolve "$A/alpha" --json > a.json; "$MJ" context resolve "$A/alpha" --json > b.json
cmp -s a.json b.json || { echo "    two resolutions differed"; exit 1; }
fp1="$(sed -n 's/.*"fingerprint":"\([0-9a-f]*\)".*/\1/p' a.json)"
printf '\nmore prose\n' >> "$A/alpha/aa.md"
"$MJ" context resolve "$A/alpha" --json > c.json
fp2="$(sed -n 's/.*"fingerprint":"\([0-9a-f]*\)".*/\1/p' c.json)"
[ -n "$fp1" ] && [ "$fp1" != "$fp2" ] || { echo "    the fingerprint did not follow the content"; exit 1; }
rm -f "$A/alpha/zz.md" "$A/alpha/aa.md" a.json b.json c.json

# ---------------------------------------------------------------- composition
# replace: the named ancestor leaves the chain, with its reason; provenance keeps it
cset "$A/alpha/deep/guide.md" composition replace
sed -i.bak 's|^order: 100$|order: 100\nsupersedes: [ai.repo.areas.alpha]|' "$A/alpha/deep/guide.md"; rm -f "$A/alpha/deep/guide.md.bak"
expect_exit 0 "$MJ" context validate
[ "$(chain $A/alpha/deep)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.alpha.deep ai.repo.areas.gamma" ] || { echo "    replace: $(chain $A/alpha/deep)"; exit 1; }
[ "$(chain $A/alpha)" = "ai.layer ai.repo ai.repo.areas ai.repo.areas.alpha" ] || { echo "    replace leaked upward: $(chain $A/alpha)"; exit 1; }
expect_exit 0 "$MJ" context explain "$A/alpha/deep"
expect_grep 'ai\.repo\.areas\.alpha .*superseded by ai\.repo\.areas\.alpha\.deep'
# final: a descendant that names it is refused, and nothing resolves until it is fixed
cset "$A/alpha/deep/guide.md" supersedes '[ai.layer]'
expect_exit 10 "$MJ" context validate
expect_grep 'illegal-override.*ai\.layer.*final'
expect_exit 10 "$MJ" context resolve "$A/alpha/deep"
expect_grep 'does not validate'
# supersession reaches only up the ancestor chain
cset "$A/alpha/deep/guide.md" supersedes '[ai.repo.areas.beta]'
expect_exit 10 "$MJ" context validate
expect_grep 'broken-reference.*ai\.repo\.areas\.beta.*not in its ancestor chain'
cset "$A/alpha/deep/guide.md" supersedes '[ai.repo.areas.alpha]'
expect_exit 0 "$MJ" context validate

# ---------------------------------------------------------------- providers and audience
cdoc "$A/beta/claude.md" ai.repo.areas.beta.claude subtree 200 'audience: [agent]'
cset "$A/beta/claude.md" providers '[claude-code]'
expect_exit 0 "$MJ" context validate
"$MJ" context resolve "$A/beta" | grep -q 'ai\.repo\.areas\.beta\.claude' || { echo "    unfiltered resolution dropped a provider-specific document"; exit 1; }
"$MJ" context resolve "$A/beta" --provider claude-code | grep -q 'ai\.repo\.areas\.beta\.claude' || { echo "    claude-code did not receive its document"; exit 1; }
expect_exit 0 "$MJ" context resolve "$A/beta" --provider agents
expect_no_grep 'ai\.repo\.areas\.beta\.claude'
expect_grep 'ai\.repo\.areas\.beta\.here'          # "*" applies to every provider
expect_exit 0 "$MJ" context explain "$A/beta" --provider agents
expect_grep 'ai\.repo\.areas\.beta\.claude .*provider agents is not among its providers'
expect_exit 0 "$MJ" context resolve "$A/beta" --audience human
expect_no_grep 'ai\.repo\.areas\.beta\.claude'
expect_exit 2 "$MJ" context resolve "$A/beta" --provider nonesuch
expect_grep 'unknown-provider'
cset "$A/beta/claude.md" providers '[nonesuch]'
expect_exit 10 "$MJ" context validate
expect_grep 'unknown-provider.*nonesuch'
rm -f "$A/beta/claude.md"

# ---------------------------------------------------------------- deprecated
cset "$A/beta/here.md" status deprecated
expect_exit 0 "$MJ" context list
expect_grep 'ai\.repo\.areas\.beta\.here .*deprecated'
expect_exit 0 "$MJ" context resolve "$A/beta"
expect_no_grep 'ai\.repo\.areas\.beta\.here'
cset "$A/beta/here.md" status active

# ---------------------------------------------------------------- refusals, each by name
bad() { # FILE EXPECTED-CLASS [SED-EXPRESSION]
  local f="$1" cls="$2"; shift 2
  cp "$f" "$f.keep"; [ $# -gt 0 ] && { sed -i.bak "$@" "$f"; rm -f "$f.bak"; }
  expect_exit 10 "$MJ" context validate || { mv "$f.keep" "$f"; return 1; }
  expect_grep "$cls" || { mv "$f.keep" "$f"; return 1; }
  mv "$f.keep" "$f"
}
f="$A/beta/guide.md"
bad "$f" 'invalid-front-matter' 's/^id: .*/id: Not-Lower/'
bad "$f" 'invalid-front-matter.*lacks title' '/^title:/d'
bad "$f" 'invalid-front-matter.*scope' 's/^scope: .*/scope: everywhere/'
bad "$f" 'invalid-front-matter.*composition' 's/^composition: .*/composition: merge/'
bad "$f" 'invalid-front-matter.*order' 's/^order: .*/order: first/'
bad "$f" 'invalid-front-matter.*paths are for scope explicit' 's/^order: .*/order: 1\npaths: [.ai]/'
bad "$f" 'unknown-key.*flavour' 's/^order: .*/order: 1\nflavour: mint/'
bad "$f" 'unsupported-schema.*context\/v2' 's|^schema: .*|schema: context/v2|'
cp "$f" "$f.keep"; awk 'BEGIN { c = 0 } /^---$/ { c++; if (c == 2) next } { print }' "$f.keep" > "$f"
expect_exit 10 "$MJ" context validate
expect_grep 'invalid-front-matter.*never closes'
mv "$f.keep" "$f"
bad "$f" 'invalid-front-matter.*does not parse' 's/^title: .*/ title: odd indentation/'
bad "$f" 'broken-reference.*tracks .nowhere' 's|^tracks: .*|tracks: [nowhere/at/all]|'
bad "$A/gamma/only.md" 'broken-reference.*paths entry' 's|^paths: .*|paths: [.ai/repo/areas/missing]|'
bad "$A/gamma/only.md" 'broken-reference.*outside the tree' 's|^paths: .*|paths: [lib]|'
cp "$A/beta/guide.md" "$A/beta/twin.md"
expect_exit 10 "$MJ" context validate
expect_grep 'ai\.repo\.areas\.beta.*duplicate-id.*guide\.md.*twin\.md'
rm -f "$A/beta/twin.md"
cset "$A/alpha/deep/guide.md" supersedes '[ai.repo.areas.nothing]'
expect_exit 10 "$MJ" context validate
expect_grep 'broken-reference.*ai\.repo\.areas\.nothing.*no document declares'
cset "$A/alpha/deep/guide.md" supersedes '[ai.repo.areas.alpha.deep]'
expect_exit 10 "$MJ" context validate
expect_grep 'cycle.*supersedes itself'
cset "$A/alpha/deep/guide.md" supersedes '[ai.repo.areas.alpha]'
# two documents in one directory superseding each other is a cycle, not a winner
cdoc "$A/alpha/deep/x.md" ai.repo.areas.alpha.deep.x subtree 100 'composition: replace' 'supersedes: [ai.repo.areas.alpha.deep.y]'
cdoc "$A/alpha/deep/y.md" ai.repo.areas.alpha.deep.y subtree 100 'composition: replace' 'supersedes: [ai.repo.areas.alpha.deep.x]'
for g in x y; do awk 'BEGIN{n=0} /^composition: extend$/ && n == 0 { n = 1; next } { print }' "$A/alpha/deep/$g.md" > t && mv t "$A/alpha/deep/$g.md"; done
expect_exit 10 "$MJ" context validate
expect_grep 'cycle'
rm -f "$A/alpha/deep/x.md" "$A/alpha/deep/y.md"
expect_exit 0 "$MJ" context validate
# a symbolic link inside the tree is refused, not followed
ln -s ../../../README.md "$A/link.md"
expect_exit 10 "$MJ" context validate
expect_grep 'link\.md.*refused-path.*symbolic link'
rm -f "$A/link.md"
# paths that leave the repository are refused before anything is read
expect_exit 15 "$MJ" context resolve ../
expect_grep 'refused-path'
expect_exit 15 "$MJ" context resolve /etc
expect_grep 'refused-path'
expect_exit 12 "$MJ" context resolve no/such/dir
expect_grep 'no such path'
expect_exit 2 "$MJ" context resolve
expect_grep 'which path'
expect_exit 2 "$MJ" context validate extra
expect_grep 'takes no path'

# ---------------------------------------------------------------- machine-readable
expect_exit 0 "$MJ" context resolve "$A/alpha/deep" --provider agents --json
printf '%s\n' "$LAST_OUT" | jq -e '.schema == "context/v1" and .target == ".ai/repo/areas/alpha/deep" and .provider == "agents"
  and (.documents | map(.id)) == ["ai.layer","ai.repo","ai.repo.areas","ai.repo.areas.alpha.deep","ai.repo.areas.gamma"]
  and (.documents[0].composition == "final") and (.documents[0].reason | test("depth 0"))
  and (.excluded | map(.id)) == ["ai.repo.areas.alpha"] and (.excluded[0].reason | test("superseded"))
  and (.fingerprint | test("^[0-9a-f]{64}$"))' >/dev/null || { echo "    resolve --json is not the documented shape: $LAST_OUT"; exit 1; }
expect_exit 0 "$MJ" context list --json
printf '%s\n' "$LAST_OUT" | jq -e '.documents | length > 5' >/dev/null || { echo "    list --json"; exit 1; }
cp "$A/beta/guide.md" "$A/beta/twin.md"
expect_exit 10 "$MJ" context validate --json
printf '%s\n' "$LAST_OUT" | jq -e 'select(.level == "FAIL") | .category == "context" and (.message | test("duplicate-id"))' >/dev/null || { echo "    validate --json"; exit 1; }
rm -f "$A/beta/twin.md"

# ---------------------------------------------------------------- read-only, and the briefing
before="$(find .ai -type f -exec shasum -a 256 {} \; | sort)"
"$MJ" context list >/dev/null; "$MJ" context resolve "$A" >/dev/null; "$MJ" context validate >/dev/null; "$MJ" context affected >/dev/null; "$MJ" context check-sync >/dev/null 2>&1 || true
after="$(find .ai -type f -exec shasum -a 256 {} \; | sort)"
[ "$before" = "$after" ] || { echo "    a context subcommand wrote into .ai/"; exit 1; }
git add -A >/dev/null; git commit -qm fixtures
"$MJ" start "work on alpha" --scope "$A/alpha" >/dev/null
expect_exit 0 "$MJ" context
expect_grep '## CONTEXT DOCUMENTS'
expect_grep 'ai\.repo\.areas\.alpha'
expect_no_grep 'ai\.repo\.areas\.beta'
