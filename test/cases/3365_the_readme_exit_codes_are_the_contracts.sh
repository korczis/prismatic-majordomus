# The README's command table states each command's exit codes as the command model declares
# them (I2963). The table says "Exit codes are a contract", and 13 of its 14 rows were wrong:
# `check` was "0 / 10" where share/commands.yaml declares [0, 2, 10, 12, 13], `init` "0 / 15"
# where it declares [0, 2, 10, 13, 15]. A script that branches on a code the README never
# names is a script nobody wrote correctly. The model is share/commands.yaml, reconciled
# against each command's MJ_EX_* constants; site/data/generated/commands.json is its
# projection, kept current by the derive.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
README="$ROOT/README.md"
MODEL="$ROOT/site/data/generated/commands.json"
expect_file "$README"; expect_file "$MODEL"

rows() { # <readme> -> "<command>\t<codes joined by space>" for each row of the command table
  awk '
    /^\| command \| answers \| writes \| exit \|$/ { on = 1; next }
    on && /^\|---/ { next }
    on && /^\| `/ {
      name = $0; sub(/^\| `/, "", name); sub(/[` ].*/, "", name)
      n = split($0, cell, " \\| "); codes = cell[n]; sub(/ *\|$/, "", codes); gsub(/ \/ /, " ", codes)
      print name "\t" codes; next
    }
    on { exit }
  ' "$1"
}
table="$(rows "$README")"
[ "$(printf '%s\n' "$table" | grep -c .)" -ge 10 ] || { echo "    the README's command table was not found: '$table'"; exit 1; }

judge() { # <rows> -> mismatches, one per line
  printf '%s\n' "$1" | while IFS="$(printf '\t')" read -r name codes; do
    want="$(jq -r --arg n "$name" '.commands[] | select(.name == $n) | .exit_codes | map(tostring) | join(" ")' "$MODEL")"
    if [ -z "$want" ]; then echo "$name: no such command in the model"; continue; fi
    [ "$codes" = "$want" ] || echo "$name: README says '$codes', the model declares '$want'"
  done
}
bad="$(judge "$table")"
if [ -n "$bad" ]; then
  echo "    the README's exit codes are not the contract:"
  printf '%s\n' "$bad" | sed 's/^/      /'
  exit 1
fi
# and the comparison is held: a row stating one code fewer is refused
mutated="$(printf '%s\n' "$table" | awk -F '\t' 'NR == 1 { sub(/ [0-9]+$/, "", $2) } { print $1 "\t" $2 }')"
[ -n "$(judge "$mutated")" ] || { echo "    a row missing a code was not told apart"; exit 1; }
exit 0
