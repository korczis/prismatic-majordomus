# majordomus-covers: none
# The README, the site's description and the social card describe the tool as it ships.
#
# Measured at 893a79be65 on 2026-10-09 (recovery finding F07, issue I2088): the README called
# the tool lightweight, said it ran "in portable shell, and never invokes a model", and called
# `mcp` and `serve` read-only. What ships: one Rust executable beside the shell tool, optional
# reasoning advisors that call a model when a plan selects them (scripts/advisor-consult), and
# capabilities that write to the repository, declared `repository_mutation`.
#
# Held here: each sentence that said otherwise stays gone from every published copy, and the
# corrected statements name what they rest on, which must exist.
. "$ROOT/test/lib.sh"

# says <root>: 0 when no published copy under <root> repeats a sentence the tree contradicts
says() {
  for f in README.md site/config.toml share/design/brand/social-card.svg; do
    [ -f "$1/$f" ] || { echo "    $f is missing"; return 2; }
    for said in 'lightweight supervisory' 'portable shell, and never invokes a model' 'are read-only processes' 'to programs, read-only,'; do
      grep -qiF "$said" "$1/$f" && { echo "    $f still says \"$said\", which the tree does not bear out"; return 1; }
    done
  done
  return 0
}
says "$ROOT" || exit 1

# What the corrected sentences point at exists.
[ -x "$ROOT/scripts/advisor-consult" ] || { echo "    the README names scripts/advisor-consult and it is not there"; exit 1; }
[ -f "$ROOT/docs/REASONING.md" ] || { echo "    the README links docs/REASONING.md and it is not there"; exit 1; }
grep -qF 'repository_mutation' "$ROOT/README.md" || { echo "    the README no longer says how a writing capability is declared"; exit 1; }
grep -rqF 'repository_mutation' "$ROOT/apps/majordomus-cli/src/capability" \
  || { echo "    the README names the effect repository_mutation and the capability registry declares none"; exit 1; }

# The mutation this case exists for: the old tagline put back into a copy is refused.
mkdir -p copy/site copy/share/design/brand
cp "$ROOT/README.md" copy/README.md
cp "$ROOT/site/config.toml" copy/site/config.toml
cp "$ROOT/share/design/brand/social-card.svg" copy/share/design/brand/social-card.svg
says copy >/dev/null || { echo "    an unmodified copy is refused"; exit 1; }
printf '%s\n' '**A lightweight supervisory control layer for AI-assisted work.**' >> copy/README.md
if says copy >/dev/null; then echo "    the old tagline put back is not refused"; exit 1; fi
