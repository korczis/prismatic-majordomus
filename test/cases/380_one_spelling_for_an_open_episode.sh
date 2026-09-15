# majordomus-covers: session
# An open episode lives at .ai/local/state/sessions-open/<key>.yaml, and two programs compute
# the key: the shell's mj_session_key, which writes the store, and the Rust
# ProviderSessionId::store_key, which reads it. They disagreed on every provider session that
# was not already a plain identifier — a space, a slash, a run of symbols, a leading dash, more
# than 64 bytes, a multibyte character — so a reader could look for a file nobody wrote.
#
# Both are held to one measured table, test/fixtures/session-keys.tsv: the Rust side by its unit
# test the_store_key_is_the_shells_spelling, the shell side here. A key made only of dots is
# guarded on both sides, because `.` and `..` survive the allow-list and name a directory.
. "$ROOT/test/lib.sh"
TABLE="$ROOT/test/fixtures/session-keys.tsv"
[ -f "$TABLE" ] || { echo "    the key table is missing: $TABLE"; exit 1; }

# the function under test, from the library the tool runs, not a copy of it
MJ_LIB_DIR="$ROOT/lib" MJ_ROOT="$PWD" MJ_STATE_DIR="$PWD/.ai/local/state"
export MJ_LIB_DIR MJ_ROOT MJ_STATE_DIR
# shellcheck source=../../lib/common.sh
. "$ROOT/lib/common.sh"

check_table() { # table -> number of mismatching rows, printed
  local table="$1" line sent key got bad=0 rows=0 tab
  tab="$(printf '\t')"
  # Whole lines, split on the first tab by expansion. `IFS=<tab> read sent key` strips a
  # leading tab, because a tab is IFS whitespace, and read the empty-provider row as sent=hand,
  # key='' — the case failed on its own reader, which is why the split is done here instead.
  while IFS= read -r line; do
    case "$line" in '#'*|'') continue ;; esac
    case "$line" in *"$tab"*) ;; *) echo "    a row without a tab: $line" >&2; bad=$((bad + 1)); continue ;; esac
    sent="${line%%"$tab"*}"; key="${line#*"$tab"}"
    rows=$((rows + 1))
    got="$(mj_session_key "$sent")"
    if [ "$got" != "$key" ]; then
      echo "    provider session [$sent]: the shell spells '$got', the table says '$key'" >&2
      bad=$((bad + 1))
    fi
  done < "$table"
  [ "$rows" -ge 9 ] || { echo "    the table yielded $rows row(s); a check over nothing has not passed" >&2; bad=$((bad + 1)); }
  printf '%s' "$bad"
}

[ "$(check_table "$TABLE")" = 0 ] || { echo "    the shell's key disagrees with the table both writers read"; exit 1; }

# a mutation must fail before a green case means anything: one wrong expectation is caught
sed 's/^a b\/c\ta-b-c$/a b\/c\ta_b_c/' "$TABLE" > mutated.tsv
cmp -s "$TABLE" mutated.tsv && { echo "    the mutation changed nothing; the case cannot prove it fails"; exit 1; }
[ "$(check_table mutated.tsv 2>/dev/null)" = 1 ] || { echo "    a wrong row in the table went unnoticed"; exit 1; }

# the guard that matters most: no key is ever a traversal or carries a separator
for hostile in "../../etc/passwd" ".." "." "a/b/c" "~/x"; do
  k="$(mj_session_key "$hostile")"
  case "$k" in */*|.|..) echo "    [$hostile] became a path, not a segment: $k"; exit 1 ;; esac
done
