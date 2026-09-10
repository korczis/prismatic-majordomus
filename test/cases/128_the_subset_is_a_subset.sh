# majordomus-covers: none
# The layer's YAML is a subset of YAML, and a subset is a narrowing.
#
# docs/SCHEMAS.md says the subset has no flow maps and no folding: it is deliberately
# *smaller* than YAML, and apps/majordomus-cli/Cargo.toml records the matching decision to
# ship no YAML crate, because src/metadata/yaml.rs reads the subset exactly as
# mj_yaml_flatten does. Both engines are ours. Nothing was asking the other question — that
# a file our engines accept is also a file YAML accepts — and nine canonical files had
# drifted out of the language while still parsing here:
#
#   an unquoted ": " inside a plain scalar    title: majordomus bench: targets ...
#   a plain scalar opening on an indicator    - `context` can carry ...   /   ... %h ...
#   nested brackets in a flow sequence        writes: [projections[].target, ...]
#
# Each reads as intended to our flatteners and is a syntax error to every other parser, so
# `share/` — which is vendored into other repositories — could not be read by the tools that
# receive it. The fix in every case is to quote the scalar: both engines strip matching
# surrounding quotes, so the value does not move.
#
# This case asks the converse question, with a parser we did not write.
. "$ROOT/test/lib.sh"

# An independent parser, or the case refuses: a gate that cannot run must not pass quietly.
if command -v ruby >/dev/null 2>&1 && ruby -ryaml -e 'exit 0' >/dev/null 2>&1; then
  parse() { ruby -ryaml -e 'YAML.unsafe_load_file(ARGV[0]) rescue (YAML.load_file(ARGV[0]))' "$1" 2>&1; }
  PARSER="ruby psych"
elif python3 -c 'import yaml' >/dev/null 2>&1; then
  parse() { python3 -c 'import sys,yaml; yaml.safe_load(open(sys.argv[1],encoding="utf-8"))' "$1" 2>&1; }
  PARSER="python pyyaml"
else
  echo "    no independent YAML parser available (need ruby with psych, or python3 with PyYAML)"
  echo "    this case proves the subset is a subset; without a second parser it proves nothing"
  exit 1
fi
echo "    parser: $PARSER"

# Every canonical YAML file: what the tool ships and what a repository authors.
files=$(cd "$ROOT" && find share .ai/repo -name '*.yaml' -type f | LC_ALL=C sort)
[ -n "$files" ] || { echo "    found no canonical YAML files to check"; exit 1; }

n=0; bad=0
for f in $files; do
  n=$((n + 1))
  if ! err=$(parse "$ROOT/$f"); then
    bad=$((bad + 1))
    printf '    %s is not YAML:\n      %s\n' "$f" "$(printf '%s' "$err" | head -2 | tr '\n' ' ')"
  fi
done

[ "$bad" = 0 ] || {
  printf '    %d of %d canonical YAML file(s) parse only under our own subset reader.\n' "$bad" "$n"
  echo '    Quote the offending scalar: both engines strip matching surrounding quotes, so'
  echo '    the value is unchanged, and the file becomes readable to every other parser.'
  exit 1
}
printf '    %d canonical YAML file(s) parse under %s\n' "$n" "$PARSER"
