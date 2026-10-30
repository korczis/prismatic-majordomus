# majordomus-covers: init doctor
# An adopter's pre-commit doctor run costs what the commit can change (I4265). Profiled on an
# adopter's fresh layer, doctor took about 14 s, a quarter of it composing the whole context
# to prove the builder, on every commit, whatever the commit staged; the hook line init
# printed did not even say where it ran.
#
#   init prints the pre-commit line with MAJORDOMUS_DOCTOR_CONTEXT=commit, and doctor reports
#   that line wired;
#   in the commit context, with nothing staged under .ai/ or in a provider file, the builder
#   is named as not read, with the command that reads it;
#   staging a change under .ai/ brings the builder back into the commit-context run;
#   outside the commit context the builder always runs.
. "$ROOT/test/lib.sh"
expect_exit 0 "$MJ" init
expect_grep 'pre-commit: MAJORDOMUS_DOCTOR_CONTEXT=commit .* doctor \|\| exit \$\?'
"$MJ" update >/dev/null
printf '#!/bin/sh\nMAJORDOMUS_DOCTOR_CONTEXT=commit %s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$MJ" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
mkdir -p src && echo a > src/a && git add . && git commit -qm base --no-verify

MAJORDOMUS_DOCTOR_CONTEXT=commit "$MJ" doctor > "$T/c1.out" 2>&1 || true
grep -q 'OK   wiring      doctor-on-commit' "$T/c1.out" || { echo "    the commit-context hook line is not reported wired:"; grep wiring "$T/c1.out"; exit 1; }
echo b > src/a && git add src/a
MAJORDOMUS_DOCTOR_CONTEXT=commit "$MJ" doctor > "$T/c2.out" 2>&1 || true
grep -q 'builder — not read in the commit hook' "$T/c2.out" || { echo "    the builder was not named as skipped for a commit outside .ai/"; grep -i builder "$T/c2.out"; exit 1; }

printf '\n' >> .ai/repo/README.md && git add .ai/repo/README.md
MAJORDOMUS_DOCTOR_CONTEXT=commit "$MJ" doctor > "$T/c3.out" 2>&1 || true
if grep -q 'builder — not read in the commit hook' "$T/c3.out"; then echo "    a commit staging .ai/ skipped the builder"; exit 1; fi

"$MJ" doctor > "$T/c4.out" 2>&1 || true
if grep -q 'not read in the commit hook' "$T/c4.out"; then echo "    the builder was skipped outside the commit context"; exit 1; fi
echo "ok: the commit hook proves the builder only when the commit can change it"
