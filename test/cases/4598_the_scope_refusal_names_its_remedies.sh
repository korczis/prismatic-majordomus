# majordomus-covers: check finish
# claims: scope-enforcement
# A path outside the task's scope is refused, and the refusal names the ways out that exist
# (I4598): a file a generator writes is admitted once .gitattributes marks it merge=derived
# (I2114), and work that grew is closed partial and started again with the wider scope, with
# in-place widening planned (I4260) and no command named before one exists. An adopter took the second route three times
# in a day for static/css/app.css and static/data/i18n.json, one attribute line away from the
# first, because nothing said so.
#
# What it must not become:
#
#   noise on a passing check. A task whose every path is in scope, or admitted as derived,
#   prints no remedy.
#
#   a nag on an honest outcome. finish --outcome partial already says the work did not
#   complete; scope only warns there, and no remedy follows.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib gen
echo a > lib/a; echo '{}' > gen/x.json
git add -A >/dev/null; git commit -qm base
"$MJ" start "t" --scope lib >/dev/null
OUT="$(mktemp "${TMPDIR:-/tmp}/mj.case.XXXXXX")"   # outside the tree: a file here would be a touched path

# ---------------------------------------------------------------- within scope: no remedy
echo b > lib/a
expect_exit 0 "$MJ" check
expect_no_grep 'merge=derived in .gitattributes'

# ---------------------------------------------------------------- outside: refused, and the remedies named
echo '{"n":1}' > gen/x.json
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +gen/x.json — outside claimed scope \(lib\)'
expect_grep 'INFO scope .* mark it merge=derived in .gitattributes and it is admitted beside that change'
expect_grep 'finish --outcome partial and start again with the wider scope; in-place widening is planned \(I4260\)'

# ---------------------------------------------------------------- the first remedy works as named
printf 'gen/** merge=derived\n' > .gitattributes
git add .gitattributes >/dev/null; git commit -qm "the generator's output is declared derived" >/dev/null
"$MJ" check > "$OUT" 2>&1 || true
# .gitattributes itself is outside lib: committed on the base branch it would be a peer's
# change; here it is the one remaining refused path, and gen/x.json is no longer among them
grep -Eq 'FAIL scope +gen/x.json' "$OUT" && { echo "    the declared derived path is still refused"; grep scope "$OUT"; exit 1; }
grep -Eq 'derived from them \(merge=derived\)|FAIL scope +\.gitattributes' "$OUT" \
  || { echo "    the derived admission did not apply"; grep scope "$OUT"; exit 1; }

# ---------------------------------------------------------------- an honest partial: warned, no remedy
NOTE="$(mktemp "${TMPDIR:-/tmp}/mj.note.XXXXXX")"
printf '# Objective\nt\n\n# Current State\npartial\n\n# Next Action\nwiden\n' > "$NOTE"
"$MJ" finish --outcome partial --note "$NOTE" > "$OUT" 2>&1 || true
grep -Eq 'WARN scope ' "$OUT" || { echo "    partial did not warn about the path outside scope"; grep -i scope "$OUT"; exit 1; }
grep -q 'merge=derived in .gitattributes' "$OUT" && { echo "    a partial finish was given the remedy"; grep -i scope "$OUT"; exit 1; }
exit 0
