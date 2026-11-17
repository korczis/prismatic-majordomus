# majordomus-covers: rules
# Front matter the tool ships is YAML every reader can parse, not only the tool's own reader.
#
# The tool reads front matter with its own line reader, which takes `description: a: b` as the
# key `description` and the text `a: b`. YAML does not: a plain scalar may not hold ": ", so a
# YAML parser refuses the whole block. Every repository that vendors the standard rules, or is
# initialised from the skeleton, then publishes them through tools that use a YAML parser, and
# those tools see no front matter at all. Seen in the OpenTTD research fork on 2026-11-17: its
# documentation site and wiki rendered five vendored rules and the skeleton's .ai/README.md
# with their fields as raw text, and the rule tables got empty cells for them. Both of the
# tool's own readers kept working, so nothing here noticed.
#
# What is held: every Markdown file under share/ that opens with front matter parses as YAML,
# and so does the .ai/README.md that `init` writes.
. "$ROOT/test/lib.sh"
python3 -I -c 'import yaml' >/dev/null 2>&1 || skip "no python3 with PyYAML: no YAML parser to ask"

parses() { # $1 file -> exit 0 when its front matter is YAML, else print the parser's reason
  python3 -I - "$1" <<'PY'
import sys, yaml
text = open(sys.argv[1], encoding="utf-8").read()
end = text.find("\n---\n", 4)
try:
    data = yaml.safe_load(text[4:end])
except yaml.YAMLError as err:
    print(" ".join(str(err).split())[:200])
    sys.exit(1)
sys.exit(0 if isinstance(data, dict) else 1)
PY
}

bad=0
while IFS= read -r f; do
  [ "$(head -n 1 "$ROOT/$f")" = "---" ] || continue
  if ! why="$(parses "$ROOT/$f")"; then
    echo "    front matter is not YAML: $f: $why"; bad=$((bad + 1))
  fi
done <<EOF
$(git -C "$ROOT" ls-files 'share/*.md')
EOF
[ "$bad" -eq 0 ] || { printf '    %s shipped file(s) carry front matter a YAML parser refuses\n' "$bad"; exit 1; }

"$MJ" init >/dev/null
why="$(parses .ai/README.md)" || { printf '    the .ai/README.md init writes is not YAML: %s\n' "$why"; exit 1; }
