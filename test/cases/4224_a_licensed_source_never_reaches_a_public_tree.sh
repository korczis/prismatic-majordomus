# majordomus-covers: none
# A licensed kit's source never reaches a public tree (I4336 (a)): `design licence`, the
# never-publish gate, compares the raw bytes of every tracked file, and of every untracked
# file under each `ui.publish` directory, with the kit-evidence/v1 fingerprints of the
# declared kit's licensed files, exactly and with every run of whitespace made one space.
#
#   1  public, a tracked file byte-identical to a licensed one: refused, naming the file and
#      the licensed source, kind exact; the --json answer says the same
#   2  the same file reformatted (CRLF, tabs): refused, kind normalised
#   3  the same tree declared private: active; MAJORDOMUS_VISIBILITY=public tightens it back
#      to refused, and the word private never loosens a public tree
#   4  public, a kit declared, a UI file, no evidence anywhere: not_judged, a non-zero exit
#   5  ui.kit "": inactive, "no kit declared", whatever the tree holds
#   6  an untracked (ignored) licensed file under a ui.publish directory: refused; so is a
#      symbolic link there into an ignored node_modules, and a file of a nested repository
#      the directory is (a gh-pages checkout), as the deploy's archive reads them; an
#      absolute ui.publish entry is not_judged, naming the entry
#   7  the policy's ~-relative ui.evidence, expanded against $HOME, judges as --evidence does;
#      an absolute entry is not_judged, naming the entry
#   8  a malformed evidence file: an error naming the file and the line, never its content
#   9  a licensed file committed and then overwritten, unstaged or staged: still refused,
#      since the commit is what a push sends
#  10  private, no evidence anywhere: inactive, exit 0
#  11  public, no evidence, no UI file tracked: inactive, has_ui false; one upper-case
#      PAGE.HTML is a UI file, and the tree is not_judged again
#
# The fixture names the fictitious kit examplekit; no real kit's bytes are in it.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_VISIBILITY MAJORDOMUS_KIT_EVIDENCE_EXAMPLEKIT 2>/dev/null || true
export HOME="$T/home"; mkdir -p "$HOME"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj4224.XXXXXX")"; trap 'rm -rf "$S"' EXIT

"$MJ" init >/dev/null 2>&1; "$MJ" update >/dev/null 2>&1
P=.ai/repo/policy.yaml
grep -q '^ui:$' "$P" || { echo "    the skeleton policy carries no ui: block"; exit 1; }
# policy <key> <value> — set one ui: key of the fixture policy, in place
policy() {
  sed -i.bak "s|^  $1: .*|  $1: $2|" "$P" && rm -f "$P.bak"
  grep -qF "  $1: $2" "$P" || { echo "    could not set ui.$1 to $2"; exit 1; }
}
# commit <message> — commit whatever changed; a policy put back as committed is no commit
commit() { git add -A >/dev/null; git diff --cached --quiet || git commit -qm "$1" >/dev/null; }
licence() { expect_exit "$1" "$RB" design licence "${@:2}"; }

# the licensed file, as the owner's kit ships it, and its kit-evidence/v1 line
mkdir -p "$S/kit/blocks"
printf '<div class="examplekit-shell">\n  <nav>pro</nav>\n</div>\n' > "$S/kit/blocks/shell.html"
exact="$(sha256_of_file "$S/kit/blocks/shell.html")"
norm="$(LC_ALL=C tr '\t\r\n' '   ' < "$S/kit/blocks/shell.html" | LC_ALL=C tr -s ' ' | mj_sha256_hex)"
EV="$S/evidence.tsv"
printf '# kit-evidence/v1\n%s\t%s\texamplekit/blocks/shell.html\n' "$exact" "$norm" > "$EV"

policy kit examplekit
mkdir -p web && cp "$S/kit/blocks/shell.html" web/shell.html
printf '<p>own page</p>\n' > web/index.html
commit 'a page built on examplekit'

# ---------------------------------------------------------------- 1. exact copy: refused
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/shell\.html holds examplekit/blocks/shell\.html \(exact copy\)$' || exit 1
expect_grep 'design licence: refused: 1 licensed file\(s\) in a public tree \(kit examplekit\)' || exit 1
expect_no_grep 'web/index\.html' || exit 1
"$RB" design licence --evidence "$EV" --json > "$S/a.json" 2>/dev/null || true
jq -e '.verdict == "refused" and .fails and .kit == "examplekit" and .visibility == "public"
  and .matches == [{"path":"web/shell.html","source":"examplekit/blocks/shell.html","kind":"exact"}]' \
  "$S/a.json" >/dev/null || { echo "    the --json answer is not the refusal:"; cat "$S/a.json"; exit 1; }

# ---------------------------------------------------------------- 2. reformatted: normalised
printf '<div class="examplekit-shell">\r\n\t<nav>pro</nav>\r\n</div>\r\n' > web/shell.html
commit 'the copy reformatted'
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/shell\.html holds examplekit/blocks/shell\.html \(normalised copy\)$' || exit 1

# ---------------------------------------------------------------- 3. private: active
policy visibility private
commit 'declared private'
licence 0 --evidence "$EV" || exit 1
expect_grep 'design licence: active: kit examplekit, private tree' || exit 1
MAJORDOMUS_VISIBILITY=public licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/shell\.html holds' || exit 1
policy visibility public
commit 'public again'
MAJORDOMUS_VISIBILITY=private licence 10 --evidence "$EV" || exit 1

# ---------------------------------------------------------------- 4. no evidence: not judged
licence 12 || exit 1
expect_grep 'design licence: not judged: a public tree declares a licensed kit and this machine holds no licence evidence' || exit 1
"$RB" design licence --json > "$S/n.json" 2>/dev/null && { echo "    not_judged exited 0"; exit 1; }
jq -e '.verdict == "not_judged" and .fails and .matches == []' "$S/n.json" >/dev/null \
  || { echo "    the --json answer is not not_judged:"; cat "$S/n.json"; exit 1; }

# ---------------------------------------------------------------- 5. no kit: inactive
policy kit '""'
commit 'no kit'
licence 0 --evidence "$EV" || exit 1
expect_grep 'design licence: inactive: no kit declared \(ui\.kit is empty\)' || exit 1
expect_no_grep '^FAIL' || exit 1
policy kit examplekit
commit 'the kit again'

# ---------------------------------------------------------------- 6. untracked under ui.publish
git rm -q web/shell.html && commit 'the copy removed'
licence 0 --evidence "$EV" || exit 1
expect_grep 'design licence: active: kit examplekit, public tree' || exit 1
printf 'dist/\n' >> .gitignore && commit 'dist is build output'
mkdir -p dist/vendor && cp "$S/kit/blocks/shell.html" dist/vendor/shell.html
# not yet a published directory: an ignored file outside ui.publish is not judged
licence 0 --evidence "$EV" || exit 1
policy publish '[dist]'
commit 'dist is what a deploy publishes'
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL dist/vendor/shell\.html holds examplekit/blocks/shell\.html \(exact copy\)$' || exit 1
# a symbolic link under dist into an ignored node_modules: the archive dereferences it
printf 'node_modules/\n' >> .gitignore && commit 'node_modules is installed'
mkdir -p node_modules/examplekit && cp "$S/kit/blocks/shell.html" node_modules/examplekit/shell.html
rm dist/vendor/shell.html && ln -s ../../node_modules/examplekit/shell.html dist/vendor/shell.html
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL dist/vendor/shell\.html holds examplekit/blocks/shell\.html \(exact copy\)$' || exit 1
expect_no_grep 'node_modules' || exit 1
# a linked directory is walked too
rm dist/vendor/shell.html && ln -s ../node_modules/examplekit dist/kit
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL dist/kit/shell\.html holds' || exit 1
rm -rf dist node_modules
# dist a repository of its own (a gh-pages checkout): git lists only dist/, the deploy all of it
mkdir dist && git -C dist init -q && cp "$S/kit/blocks/shell.html" dist/page.html
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL dist/page\.html holds examplekit/blocks/shell\.html \(exact copy\)$' || exit 1
expect_no_grep 'dist/\.git' || exit 1
rm -rf dist && mkdir -p dist/vendor && cp "$S/kit/blocks/shell.html" dist/vendor/shell.html
policy publish '["/abs/dist"]'
licence 12 --evidence "$EV" || exit 1
expect_grep "design licence: not judged: the policy's ui: block cannot be used: ui\.publish entry \`/abs/dist\` is not a repository-relative directory" || exit 1
"$RB" design licence --evidence "$EV" --json > "$S/p.json" 2>/dev/null && { echo "    an unusable ui.publish exited 0"; exit 1; }
jq -e '.verdict == "not_judged" and .fails and (.reason | test("ui.publish entry"))' "$S/p.json" >/dev/null \
  || { echo "    the --json answer is not not_judged:"; cat "$S/p.json"; exit 1; }
policy publish '[dist]'
commit 'dist again'

# ---------------------------------------------------------------- 7. ui.evidence under $HOME
mkdir -p "$HOME/.config/examplekit" && cp "$EV" "$HOME/.config/examplekit/evidence.tsv"
policy evidence '["~/.config/examplekit/evidence.tsv"]'
commit 'the owner evidence named'
licence 10 || exit 1
expect_grep '^FAIL dist/vendor/shell\.html holds examplekit/blocks/shell\.html' || exit 1
"$RB" design licence --json > "$S/h.json" 2>/dev/null || true
jq -e '.evidence == ["~/.config/examplekit/evidence.tsv"] and .fingerprints == 1' "$S/h.json" >/dev/null \
  || { echo "    the answer does not name the ui.evidence entry:"; cat "$S/h.json"; exit 1; }
policy evidence "[\"$HOME/.config/examplekit/evidence.tsv\"]"
licence 12 || exit 1
expect_grep "not judged: the policy's ui: block cannot be used: ui\.evidence entry .* is not \`~/\`-relative" || exit 1
policy evidence '[]'
commit 'no evidence named'

# ---------------------------------------------------------------- 8. malformed: named, never shown
BAD="$S/bad-evidence.tsv"
printf '# kit-evidence/v1\n%s\tSECRET-EVIDENCE-VALUE\n' "$exact" > "$BAD"
licence 2 --evidence "$BAD" || exit 1
expect_grep "$BAD: line 2: not kit-evidence/v1: a digest is not 16 or 64 hex digits" || exit 1
expect_no_grep 'SECRET-EVIDENCE-VALUE' || exit 1
expect_no_grep "$exact" || exit 1
printf 'one-column-SECRET-EVIDENCE-VALUE\n' > "$BAD"
licence 2 --evidence "$BAD" || exit 1
expect_grep "$BAD: line 1: not kit-evidence/v1: fewer than two tab-separated columns" || exit 1
expect_no_grep 'SECRET-EVIDENCE-VALUE' || exit 1
licence 2 --evidence "$S/absent.tsv" || exit 1
expect_grep "$S/absent\.tsv: " || exit 1

# ---------------------------------------------------------------- 9. committed, then overwritten
rm -rf dist
cp "$S/kit/blocks/shell.html" web/page.html && commit 'the licensed page committed'
printf '<p>other</p>\n' > web/page.html
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/page\.html holds examplekit/blocks/shell\.html \(exact copy\)$' || exit 1
git add web/page.html
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/page\.html holds' || exit 1
commit 'the page replaced'
licence 0 --evidence "$EV" || exit 1
# and a copy only in the working copy is caught before it is staged
cp "$S/kit/blocks/shell.html" web/page.html
licence 10 --evidence "$EV" || exit 1
expect_grep '^FAIL web/page\.html holds' || exit 1
git checkout -q -- web/page.html

# ---------------------------------------------------------------- 10. private, no evidence: inactive
policy visibility private
commit 'private, no evidence'
licence 0 || exit 1
expect_grep 'design licence: inactive: a private tree, and no licence evidence on this machine' || exit 1
policy visibility public
commit 'public, no evidence'
licence 12 || exit 1

# ---------------------------------------------------------------- 11. no UI file: inactive
git ls-files -z -- '*.html' '*.htm' '*.heex' '*.jsx' '*.tsx' '*.vue' '*.svelte' | xargs -0 git rm -q --
commit 'no UI file'
licence 0 || exit 1
expect_grep 'design licence: inactive: no UI file is tracked, so the kit is not held here' || exit 1
"$RB" design licence --json > "$S/u.json" 2>/dev/null || { echo "    no UI file did not exit 0"; exit 1; }
jq -e '.verdict == "inactive" and (.fails | not) and .has_ui == false' "$S/u.json" >/dev/null \
  || { echo "    the --json answer is not inactive without UI:"; cat "$S/u.json"; exit 1; }
mkdir -p web && printf '<p>upper</p>\n' > web/PAGE.HTML && commit 'an upper-case UI file'
licence 12 || exit 1
"$RB" design licence --json > "$S/v.json" 2>/dev/null || true
jq -e '.has_ui == true and .verdict == "not_judged"' "$S/v.json" >/dev/null \
  || { echo "    PAGE.HTML is not a UI file:"; cat "$S/v.json"; exit 1; }
exit 0
