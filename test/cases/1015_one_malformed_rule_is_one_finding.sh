# majordomus-covers: rules doctor
# majordomus-negative: rules
# One rule file whose front matter the parser refuses is one finding, not one per rule.
#
# The manifest check reads every rule's identity in one batched pipeline and, when the
# parser refuses a front matter, falls back to one pipeline per file. The fallback was
# guarded by `if ! a | parser | awk`, and without pipefail that `if` saw only the last awk,
# which always succeeds: the fallback never ran. One malformed rule then cut the batch short
# and every identity after it was read as `@` — measured on 2026-10-09 against the shipped
# package with one tab-indented front-matter line: 57 findings where there was one. doctor
# runs the same check over every adopter's vendored rules, and neither doctor nor
# scripts/rules-package set pipefail, so the check is held here without it.
. "$ROOT/test/lib.sh"
pkg="$T/pkg"; rm -rf "$pkg"; cp -R "$ROOT/share/standard/majordomus" "$pkg"
f="$pkg/rules/blocker-resolution.v1.md"
[ -f "$f" ] || { echo "    the fixture rule is gone from the package"; exit 1; }
awk 'NR == 2 { print; print "\tstray: a tab-indented line the parser refuses"; next } { print }' "$f" > "$f.new" \
  && mv "$f.new" "$f"

check() {  # the manifest check exactly as doctor calls it, in a shell without pipefail
  bash -c 'set -eu; MJ_VERSION=x MJ_BIN_DIR="$1/bin" MJ_LIB_DIR="$1/lib"; export MJ_VERSION MJ_BIN_DIR MJ_LIB_DIR
           . "$1/lib/common.sh"; . "$1/lib/rules.sh"; mj_rules_manifest_check "$2" || true' _ "$ROOT" "$pkg" 2>/dev/null
}
out="$(check)"
n="$(printf '%s\n' "$out" | sed '/^$/d' | wc -l | tr -d ' ')"
[ "$n" = 1 ] || { echo "    one malformed rule produced $n findings:"; printf '%s\n' "$out" | head -4 | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q '^rules/blocker-resolution.v1.md ' \
  || { echo "    the one finding does not name the malformed rule: $out"; exit 1; }
if printf '%s\n' "$out" | grep -q 'declares @'; then
  echo "    a rule was read as declaring nothing: $out"; exit 1
fi

# and the package as shipped is clean: the change did not move the check's verdict on it
rm -rf "$pkg"; cp -R "$ROOT/share/standard/majordomus" "$pkg"
out="$(check)"
[ -z "$out" ] || { echo "    the shipped package now has findings: $out"; exit 1; }
