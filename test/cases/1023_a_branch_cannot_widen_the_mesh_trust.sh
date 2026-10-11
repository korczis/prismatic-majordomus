# majordomus-covers: none
# majordomus-timeout: 120
# A branch cannot widen the mesh's trust: the declaration in force is the working tree's with
# every widening of the trunk's committed copy removed (I2135, ADR 0050 rule 6).
#
# The server read .ai/repo/mesh/*.yaml from the working tree, so a contributor's branch that
# added a key, a hub address or `policy: tofu` was obeyed by the runtime of whoever checked it
# out to review it. Here the trunk commits a declaration that trusts one key; a branch commits
# one that trusts a second key, trusts on first use and names a host of its own; and the
# doctor of that checkout must describe the trunk's trust, and name what it ignores.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
gitq() { git -c user.email=t@example.com -c user.name=t "$@"; }
A=946e8aa593fe16c58d5eb43493338810896cd10bc6bdd44ffa8e9bfebd37ca1f
B=3f7b357805aa15ecdb3c1ace7e7ac02de263e3d0def8aafda9ad280d3af7c813

"$MJ" init >/dev/null
gitq add -A >/dev/null && gitq commit -qm layer
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - %s\n' "$A" > .ai/repo/mesh/majordomus.yaml
gitq add -A && gitq commit -qm "the mesh, as reviewed"

gitq checkout -q -b contribution
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\nrendezvous:\n  endpoints:\n    - http://198.51.100.7:8791\ntrust:\n  policy: tofu\n  allow:\n    - %s\n    - %s\n' "$A" "$B" > .ai/repo/mesh/majordomus.yaml
gitq add -A && gitq commit -qm "trust one more machine"

out="$("$RB" run mesh.doctor --format json 2>/dev/null)" || true
checks="$(printf '%s' "$out" | jq -c '.output.checks // .checks')" \
  || { echo "    mesh.doctor did not answer JSON:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
described="$(printf '%s' "$checks" | jq -r '.[] | select(.check == "declaration") | .detail')"
case "$described" in
  *"trust=deny_unknown (1 allowed key(s))"*"rendezvous endpoints=0"*|*"rendezvous endpoints=0"*"trust=deny_unknown (1 allowed key(s))"*) ;;
  *) echo "    the branch's widening is in force: $described"; exit 1 ;;
esac
root="$(printf '%s' "$checks" | jq -c '.[] | select(.check == "trust-root")')"
printf '%s' "$root" | jq -e '.ok == false' >/dev/null \
  || { echo "    the doctor does not say what this checkout ignores: $root"; exit 1; }
for named in "$B" 198.51.100.7 tofu; do
  printf '%s' "$root" | jq -r .detail | grep -qF "$named" \
    || { echo "    the trust-root check does not name $named: $root"; exit 1; }
done
exit 0
