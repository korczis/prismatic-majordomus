# majordomus-covers: none
# A quoted scalar reads as YAML reads it (I3011). Case 128 asks whether a canonical file is
# YAML at all; this one asks whether a value in it means what YAML says it means. The shell
# flattener and the Rust engine stripped a double-quoted scalar's quotes and kept its escapes,
# while every writer here escapes as JSON, so `echo \"held\"` was published for `echo "held"`
# and a quote, a backslash or a slash in 112 escapes of the plan read back wrong.
#
# For every canonical YAML file, every string value an independent parser reads with a quote
# or a backslash in it (where an escape can matter) must be the value mj_yaml_flatten reads at
# the same path. A value holding a newline or a tab is left out: the flattener writes one line
# per value and keeps those escapes as written, which docs/SCHEMAS.md states. The Rust engine
# is held to the same reading by metadata::yaml's a_double_quoted_scalar_reads_its_escapes.
. "$ROOT/test/lib.sh"
. "$ROOT/lib/common.sh"

python3 -c 'import yaml' >/dev/null 2>&1 || {
  echo "    PyYAML is not available; this case compares against an independent parser and"
  echo "    without one it proves nothing"
  exit 1
}

files=$(cd "$ROOT" && find share .ai/repo -name '*.yaml' -type f | LC_ALL=C sort)
[ -n "$files" ] || { echo "    found no canonical YAML files to check"; exit 1; }

mkdir -p flat
n=0
for f in $files; do
  n=$((n + 1))
  mj_yaml_flatten "$ROOT/$f" > "flat/$n" 2>/dev/null || : > "flat/$n"
  printf '%s\t%s\n' "$n" "$f" >> index.tsv
done

python3 - "$ROOT" index.tsv flat <<'PY' > findings.txt
import os, sys, yaml
root, index, flat = sys.argv[1:4]

def walk(v, path, out):
    if isinstance(v, dict):
        for k, x in v.items():
            walk(x, f"{path}.{k}" if path else str(k), out)
    elif isinstance(v, list):
        for i, x in enumerate(v):
            walk(x, f"{path}.{i}" if path else str(i), out)
    elif isinstance(v, str):
        out[path] = v

checked = 0
for line in open(index, encoding="utf-8"):
    n, rel = line.rstrip("\n").split("\t", 1)
    try:
        doc = yaml.safe_load(open(os.path.join(root, rel), encoding="utf-8"))
    except yaml.YAMLError:
        continue  # case 128 refuses a file that is not YAML
    want = {}
    walk(doc, "", want)
    got = {}
    for row in open(os.path.join(flat, n), encoding="utf-8", errors="replace"):
        k, sep, v = row.rstrip("\n").partition("=")
        if sep:
            got[k] = v
    for path, value in want.items():
        if not ('"' in value or "\\" in value) or "\n" in value or "\t" in value:
            continue
        checked += 1
        if got.get(path) != value:
            print(f"{rel}:{path}: YAML reads {value!r}, the flattener {got.get(path)!r}")
print(f"#checked {checked}")
PY

checked=$(sed -n 's/^#checked //p' findings.txt)
bad=$(grep -vc '^#checked ' findings.txt || true)
if [ "$bad" != 0 ]; then
  grep -v '^#checked ' findings.txt | head -20 | sed 's/^/    /'
  echo "    $bad value(s) of $checked read otherwise than YAML reads them"
  exit 1
fi
[ "${checked:-0}" -gt 0 ] || { echo "    no value with a quote or a backslash was checked; the case proved nothing"; exit 1; }
echo "    $checked value(s) with a quote or a backslash, in $n canonical YAML file(s), read as YAML reads them"
