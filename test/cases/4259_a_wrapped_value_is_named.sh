# majordomus-covers: plan knowledge
# A wrapped value is named for what it is, by both parsers of the YAML subset (I4259). An
# adopter's tool wrote front matter with PyYAML, which folds a long value onto the next line,
# and was told "cannot parse line 2: overview of the UI stack)": a fragment and no cause.
#
#   the shell's parser names the line and the key whose value it continues, with the remedy;
#   the executable's parser says the same of a document's front matter;
#   a line that is no key and continues nothing keeps the plain "cannot parse" message;
#   a document's own key in the x- namespace is indexed, and any other unknown key is refused.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
pj_init
pj_milestone m0 0
printf 'risks:\n  - one\n' >> .ai/repo/project/milestones/m0.yaml
pj_issue I0001 m0
# a value an emitter wrapped onto the next line
sed -i.bak 's/^problem: "A problem worth solving."$/problem: A problem worth solving that a standard emitter\n  wrapped onto a second line/' .ai/repo/project/milestones/m0.yaml && rm -f .ai/repo/project/milestones/m0.yaml.bak
git add . && git commit -qm plan
expect_exit 10 "$MJ" plan validate
expect_grep "continues the value of 'problem' from the line above"
expect_grep 'join it onto that line'

# a stray line that continues nothing is still a plain parse failure
pj_milestone m0 0
printf 'not a key at all\n' >> .ai/repo/project/milestones/m0.yaml
git add . && git commit -qm stray
expect_exit 10 "$MJ" plan validate
expect_grep 'cannot parse line'
expect_no_grep 'continues the value'
pj_milestone m0 0; git add . && git commit -qm fixed

RB="$(rust_bin 2>/dev/null)" || skip "the executable is not built here"
[ -x "$RB" ] || skip "the executable is not built here"
mkdir -p docs
printf -- '---\ndescription: Flowbite rules and component index for LLM-assisted UI work (see the\n  overview of the UI stack)\nupdated: 2026-10-27\n---\n\n# Reference\n' > docs/reference.md
git add . && git commit -qm doc
MAJORDOMUS_BIN="$RB" "$MJ" knowledge validate > "$T/kv.out" 2>&1 || true
grep -q "continues the value of 'description' from the line above" "$T/kv.out" \
  || { echo "    the executable did not name the wrapped value:"; grep -i 'reference.md' "$T/kv.out"; exit 1; }

# the adopter's provenance keys are its own; a misspelt contract key is still refused
printf -- '---\ndescription: A fetched snapshot of a reference\nx-source: https://example.org/reference\nx-fetched: 2026-10-27\n---\n\n# Reference\n' > docs/reference.md
printf -- '---\ntitel: A misspelt key\n---\n\n# Other\n' > docs/other.md
git add . && git commit -qm extension
MAJORDOMUS_BIN="$RB" "$MJ" knowledge validate > "$T/kx.out" 2>&1 || true
if grep -q 'docs/reference.md' "$T/kx.out"; then
  echo "    a document's x- keys were refused:"; grep 'docs/reference.md' "$T/kx.out"; exit 1
fi
grep 'docs/other.md' "$T/kx.out" | grep -q 'goes in the x- namespace' \
  || { echo "    an unknown key was not refused with the x- remedy"; cat "$T/kx.out"; exit 1; }
echo "ok: both parsers name a wrapped value, keep the plain message for a stray line, and admit x- keys only"
