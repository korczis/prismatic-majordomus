# majordomus-covers: doctor context usecase
# majordomus-negative: doctor context usecase
# The catalogue, DAG, context and command-surface validators read their inputs once (I0820):
# the work each does is a fixed number of processes whatever the number of records, and the
# findings are the ones the per-record loops printed, byte for byte, failures included.
#
# The proofs count work, not time: MJ_TIMING's counters are exact on any machine, where a
# wall-clock bound measures the host. Each count is taken twice, on a small tree and on a
# larger one, and the two must be equal; a validator that went back to one process per
# record would make the second larger.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && printf 'a\n' > lib/a.sh && printf 'b\n' > lib/b.sh && git add -A && git commit -qm base
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH

# count NAME FILE: a counter of the timing report on FILE, 0 when absent
count() { awk -v n="$1" '$1 == "count" && $3 == n { print $2; f = 1 } END { if (!f) print 0 }' "$2"; }
# tracks FILE SPEC...: give one context document's front matter these tracked pathspecs
tracks() {
  local f="$1"; shift
  local list; list="$(printf '%s, ' "$@")"; list="${list%, }"
  grep -q '^tracks:' "$f" && sed -i.bak '/^tracks:/d' "$f" && rm -f "$f.bak"
  awk -v l="tracks: [$list]" 'NR > 1 && $0 == "---" && !d { print l; d = 1 } { print }' "$f" > "$f.new" && mv "$f.new" "$f"
}

# ---------------------------------------------------------------- context: tracks, one git call
R1=.ai/repo/rules/README.md; R2=.ai/README.md
[ -f "$R1" ] && [ -f "$R2" ] || { echo "    the seeded layer has no $R1 or $R2"; exit 1; }
tracks "$R1" lib/a.sh
expect_exit 0 "$MJ" context validate
MJ_TIMING=1 "$MJ" context validate >/dev/null 2>"$T/t1"
g1="$(count git "$T/t1")"
# five pathspecs over two documents, one of each kind of answer: a file, a directory, a
# glob, one that only an untracked file matches, and one that matches nothing
printf 'u\n' > lib/untracked.txt
tracks "$R1" lib/a.sh lib 'lib/*.sh'
tracks "$R2" lib/untracked.txt nope/missing
git add "$R1" "$R2" && git -c core.hooksPath=/dev/null commit -qm tracks
expect_exit 10 "$MJ" context validate
expect_grep "broken-reference: tracks 'lib/untracked.txt', which matches no tracked file"
expect_grep "broken-reference: tracks 'nope/missing', which matches no tracked file"
expect_no_grep "tracks 'lib/a.sh'"
expect_no_grep "tracks 'lib', "
expect_no_grep "tracks 'lib/\\*.sh'"
# the two findings in the order the documents and their pathspecs are declared
[ "$(printf '%s\n' "$LAST_OUT" | grep -c 'which matches no tracked file')" = 2 ] || { echo "    expected exactly two unmatched pathspecs"; exit 1; }
MJ_TIMING=1 "$MJ" context validate >/dev/null 2>"$T/t2" || :
g2="$(count git "$T/t2")"
[ "$g1" = "$g2" ] || { echo "    context validate ran $g1 git process(es) for one pathspec and $g2 for five"; exit 1; }
# a magic pathspec is still asked on its own and still answered
tracks "$R2" ':(glob)lib/*.sh'
expect_exit 0 "$MJ" context validate
tracks "$R2" ':(glob)nope/*.sh'
expect_exit 10 "$MJ" context validate
expect_grep "tracks ':\\(glob\\)nope/\\*.sh', which matches no tracked file"
git checkout -q -- "$R1" "$R2" 2>/dev/null || git checkout -q HEAD~1 -- "$R1" "$R2"
tracks "$R1" lib/a.sh; tracks "$R2" lib/b.sh
rm -f lib/untracked.txt
expect_exit 0 "$MJ" context validate

# a Markdown file that is not a context document stays out of it, and one the manifest
# names as a convention is still refused when it carries no contract
printf -- '---\nkind: rule\n---\n# not context\n' > .ai/repo/notes.md
printf 'no front matter\n' > .ai/repo/plain.md
expect_exit 0 "$MJ" context validate
mkdir -p .ai/repo/stray && printf '# stray\n' > .ai/repo/stray/README.md
expect_exit 10 "$MJ" context validate
expect_grep 'stray/README\.md.*invalid-front-matter.*manifest names README\.md'
rm -rf .ai/repo/stray .ai/repo/notes.md .ai/repo/plain.md

# ---------------------------------------------------------------- the catalogue: one pass a section
UCD=.ai/repo/use-cases
# uc ID: a use case that resolves against the seeded layer
uc() {
  printf -- '---\nid: %s\nkind: use-case\ntitle: Use case %s\nsummary: What %s does.\ncategory: adoption\nstatus: active\ntarget: advisory\ncommands: [doctor]\n---\n\n# Situation\n\nA reader.\n\n# Scenario\n\nDescribed.\n\n# Outcome\n\nDone.\n' "$1" "$1" "$1" > "$UCD/$1.md"
}
sid=uc-seed; uc "$sid"
expect_exit 0 "$MJ" usecase validate
MJ_TIMING=1 "$MJ" usecase validate >/dev/null 2>"$T/u1"
for n in 1 2 3 4 5 6; do uc "$sid-copy-$n"; done
expect_exit 0 "$MJ" usecase validate
MJ_TIMING=1 "$MJ" usecase validate >/dev/null 2>"$T/u2"
for c in yaml_flatten yaml_flatten_many; do
  [ "$(count "$c" "$T/u1")" = "$(count "$c" "$T/u2")" ] \
    || { echo "    usecase validate counted $c $(count "$c" "$T/u1") times over one use case and $(count "$c" "$T/u2") over seven"; exit 1; }
done
# every mutation is still refused by name, and only on the use case that carries it
C="$UCD/$sid-copy-3.md"
awk '{ print } NR == 2 { print "bogus_key: 1" }' "$C" > "$C.new" && mv "$C.new" "$C"
grep -q '^bogus_key: 1$' "$C" || { echo "    the probe did not take"; exit 1; }
sed -i.bak 's/^# Outcome[[:space:]]*$/# Result/' "$C"; rm -f "$C.bak"
grep -q '^# Result$' "$C" || { echo "    the heading probe did not take"; exit 1; }
expect_exit 10 "$MJ" usecase validate
expect_grep "use-case +$sid-copy-3 — unknown key\\(s\\): bogus_key"
expect_grep "use-case +$sid-copy-3 — body has no '# Outcome' heading"
expect_no_grep "copy-2 — "
# a front matter that does not parse leaves its file without a record, by name
printf -- '---\nid: broken\n\tkind: use-case\n---\n' > "$UCD/broken.md"
expect_exit 10 "$MJ" usecase validate
expect_grep 'broken\.md — front matter does not parse or declares no id'
rm -f "$UCD/broken.md" "$UCD/$sid".md "$UCD/$sid"-copy-*.md
expect_exit 0 "$MJ" usecase validate

# ---------------------------------------------------------------- the DAG: the loop's lines
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
sed -i.bak '/^evidence_required:/,$d' .ai/repo/project/issues/I0002.yaml; rm -f .ai/repo/project/issues/I0002.yaml.bak
git add -A && git -c core.hooksPath=/dev/null commit -qm plan
expect_exit 0 "$MJ" doctor
expect_grep '^WARN dag         I0002 — requires no evidence; nothing gates its completion  \[reproduce: majordomus plan show I0002\]$'
expect_grep '^OK   dag         [0-9]+ wave\(s\) — acyclic'
expect_exit 0 env MJ_JSON=1 "$MJ" doctor --json
expect_grep '^\{"level":"WARN","category":"dag","subject":"I0002","message":"requires no evidence; nothing gates its completion","reproduce":"majordomus plan show I0002"\}$'
# a failure is the doctrine's: FAIL under doctor, with the graph's own count after it
pj_issue I0009 M000 I9999
expect_exit 10 "$MJ" doctor
expect_grep '^FAIL dag         I0009 — depends on I9999, which is not an issue  \[reproduce: majordomus plan validate\]$'
expect_grep '^FAIL dag         graph — 1 dependency-graph failure\(s\)'
expect_no_grep '^OK   dag '
rm -f .ai/repo/project/issues/I0009.yaml

# ---------------------------------------------------------------- the command surface
expect_exit 0 "$MJ" doctor
expect_grep '^OK   command     surface — [0-9]+ public command\(s\), reconciled against the dispatch table'
