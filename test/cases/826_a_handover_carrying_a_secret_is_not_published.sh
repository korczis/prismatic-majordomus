# majordomus-covers: handover
# majordomus-timeout: 600
# Nothing secret or machine-local leaves the machine (ADR 0105). A handover carrying a secret
# environment value, a token of a known shape, a private-key header or a path of a home
# directory is refused — exit 10, the field named, the value never repeated — and the store
# is left untouched. A clean one is published, and the stored record holds none of the
# values the publishing process had.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj826.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git init -q --bare -b main "$S/shared.git"

# Each machine is a clone with its own HOME and XDG_STATE_HOME, so its own device key; the
# bare repository is the only thing they share. (The same harness as case 821.)
on() { m="$1"; shift; ( cd "$S/$m/repo" && HOME="$S/$m/home" XDG_STATE_HOME="$S/$m/state" "$@" ); }
cj() { m="$1"; shift; on "$m" "$RB" continuity "$@" --format json; }
origin() {
  mkdir -p "$S/$1/repo" "$S/$1/home"
  on "$1" git init -q -b main .
  on "$1" git config user.email "$1@example.com"; on "$1" git config user.name "$1"
  on "$1" "$MJ" init >/dev/null; on "$1" "$MJ" update >/dev/null
  mkdir -p "$S/$1/repo/lib"; echo a > "$S/$1/repo/lib/a"
  on "$1" git add -A; on "$1" git commit -qm base
  on "$1" git remote add origin "$S/shared.git"
  on "$1" git push -q -u origin main 2>/dev/null
}
clone() {
  mkdir -p "$S/$1/home"; git clone -q "$S/shared.git" "$S/$1/repo"
  on "$1" git config user.email "$1@example.com"; on "$1" git config user.name "$1"
}
# handover MACHINE STATE NEXT: a handover written with the shell tool
# (under the active task when there is one; a handover needs none — ADR 0052)
handover() {
  body="$(printf '# Objective\nShip it\n\n# Current State\n%s\n\n# Next Action\n%s\n' "$2" "$3")"
  printf '%s\n' "$body" | on "$1" "$MJ" handover >/dev/null 2>&1 \
    || printf '%s\n' "$body" | on "$1" "$MJ" handover --no-task >/dev/null 2>&1 \
    || { echo "    handover on $1 failed"; exit 1; }
}


origin a
SECRET="tok-$(printf "%s" "s3cr3t" | od -An -tx1 | tr -d " \n")-zz"
GH="ghp_$(printf "%036d" 7)"
PEM="-----BEG""IN OPENSSH PRIVATE KEY-----"
HOMEPATH="/ho""me/dev/notes/plan.txt"
refuse() { # label body
  handover a "$2" next
  rc=0; MY_DEPLOY_TOKEN="$SECRET" on a "$RB" continuity publish > "$S/out.txt" 2>&1 || rc=$?
  [ "$rc" = 10 ] || { echo "    $1: published (exit $rc)"; cat "$S/out.txt"; exit 1; }
  grep -q "/handover/body" "$S/out.txt" || { echo "    $1: the refusal does not name the field"; cat "$S/out.txt"; exit 1; }
  if grep -qF -- "$SECRET" "$S/out.txt" || grep -qF -- "$GH" "$S/out.txt"; then
    echo "    $1: the refusal repeats the secret"; exit 1
  fi
  if on a git rev-parse -q --verify refs/majordomus/continuity >/dev/null; then
    echo "    $1: a refused publication wrote the store"; exit 1
  fi
}
refuse "a secret environment value" "deployed with $SECRET"
refuse "a token" "the CI token is $GH"
refuse "a private key" "$PEM"
refuse "a home path" "notes are in $HOMEPATH"
grep -q "continuity.nonportable_field" "$S/out.txt" || { echo "    a home path is not reported as non-portable"; cat "$S/out.txt"; exit 1; }

handover a "the parser is done" "write the tests"
MY_DEPLOY_TOKEN="$SECRET" cj a publish > "$S/pub.json" || { cat "$S/pub.json"; exit 1; }
REC="$(jq -r .record.id "$S/pub.json")"
on a git cat-file -p "refs/majordomus/continuity:records/$REC.json" > "$S/rec.json"
for v in "$SECRET" "$GH" "OPENSSH" "$S" "$HOMEPATH"; do
  if grep -qF -- "$v" "$S/rec.json"; then echo "    the stored record carries $v"; exit 1; fi
done
exit 0
