# majordomus-covers: none
# claim: session-lifecycle
# The self-report is declared once (I4390, ADR 0136 amended 2026-10-29).
#
# On 2026-10-29 the owner read the interval prompt and called it a placeholder. The same
# contract was then worded four ways: brief.rs `keep_awake`, the mod's `selfReport`,
# docs/BRIEF.md and ADR 0136, each a little differently, and none of them measured. It is now
# declared in share/brief/report.yaml alone, `brief.session` composes every text from it, and
# every carrier relays those texts. A second copy is how the four wordings came about, so this
# case refuses one wherever it appears.
#
# What it proves:
#   1  every sentence of share/brief/report.yaml — each start sentence, rule text, section
#      heading, holds and evidence, attention fact longer than 20 characters, and remedy and
#      live-read command longer than 30 — appears in the tracked tree only in the declaration,
#      its compiled copy, the reader page rendered from it, the site's projection of that page
#      and the site's translation memory. On 2026-10-30 the start text was prose kept in Rust and
#      docs/BRIEF.md restated the live reads in a column of its own; neither was a rule or a
#      `holds`, so this case could not see them
#   2  the placeholder wording the owner refused is in no tracked file but ADR 0136's dated
#      history and the tests that hold its absence
#   3  in a fixture, a rule pasted into docs/BRIEF.md is refused, by path
. "$ROOT/test/lib.sh"
fail() { printf '    %s\n' "$*"; exit 1; }
DECL=share/brief/report.yaml
[ -f "$ROOT/$DECL" ] || fail "$DECL is missing"

# The sentences that are the contract, read from the declaration (one double- or single-quoted
# scalar per line, the layer's YAML subset): every start sentence; every quoted text, holds,
# heading, evidence and fact longer than 20 characters; every quoted remedy and command longer
# than 30, since a short command (`majordomus peers`) is everyone's to name.
sentences() { # root
  awk '
    /^[a-z_]+:/ { in_start = ($0 ~ /^start:/) }
    in_start && /^  - / {
      line = $0; sub(/^  - /, "", line)
      q = substr(line, 1, 1); line = substr(line, 2, length(line) - 2)
      if (q == "\047") gsub(/\047\047/, "\047", line)
      if (length(line) > 20) print line
      next
    }
    /^    (text|holds|heading|evidence|fact|remedy|command): / {
      key = $0; sub(/^ +/, "", key); sub(/:.*$/, "", key)
      line = $0; sub(/^    [a-z_]+: /, "", line)
      q = substr(line, 1, 1)
      if (q != "\"" && q != "\047") next
      line = substr(line, 2, length(line) - 2)
      if (q == "\047") gsub(/\047\047/, "\047", line)
      min = (key == "remedy" || key == "command") ? 30 : 20
      if (length(line) > min) print line
    }' "$1/$DECL"
}

# Where a sentence of the contract may stand.
allowed() {
  case "$1" in
    share/brief/report.yaml) return 0 ;;
    apps/majordomus-cli/src/capability/builtin/brief/report.yaml) return 0 ;;
    docs/generated/brief-report.md) return 0 ;;
    site/content/docs/generated/brief-report.md|site/content/docs/brief-report.md) return 0 ;;
    site/i18n/*/memory/*) return 0 ;;
  esac
  return 1
}

# check <root>: every tracked file that carries a sentence of the contract and may not
check() {
  root="$1"; bad=0
  n=0
  while IFS= read -r s; do
    [ -n "$s" ] || continue
    n=$((n + 1))
    for f in $(git -C "$root" grep -lF -- "$s" 2>/dev/null || true); do
      allowed "$f" && continue
      printf '    %s carries a sentence of %s: %s\n' "$f" "$DECL" "$(printf '%.70s' "$s")"
      bad=1
    done
  done <<EOF
$(sentences "$root")
EOF
  [ "$n" -gt 40 ] || { printf '    only %s sentences were read from %s\n' "$n" "$DECL"; return 1; }
  return "$bad"
}

# ---------------------------------------------------------------- 1  the tree holds one copy
check "$ROOT" || fail "the report contract is restated outside its declaration"

# ---------------------------------------------------------------- 2  the refused wording
for banned in 'two to four lines' 'a board that could not be read, a superseded server'; do
  for f in $(git -C "$ROOT" grep -lF -- "$banned" || true); do
    case "$f" in
      # the dated history of the decision, and its site projection
      .ai/repo/adrs/0136-every-session-briefs-itself-on-an-interval.md|site/content/adrs/adr-0136.md) ;;
      # the translation memory keeps what the site once said
      site/i18n/*/memory/*) ;;
      # the tests that hold its absence
      test/cases/4401_the_report_contract_is_declared_once.sh|test/cases/1065_a_session_briefs_itself.sh) ;;
      apps/majordomus-cli/tests/brief.rs|apps/majordomus-cli/src/capability/builtin/brief.rs) ;;
      share/mods/claude-code/majordomus-brief/hooks/register.test.ts) ;;
      *) fail "$f still says '$banned'" ;;
    esac
  done
done

# ---------------------------------------------------------------- 3  a pasted rule is refused
F="$T/fixture"; mkdir -p "$F/share/brief" "$F/docs"
git -C "$F" init -q .
cp "$ROOT/$DECL" "$F/$DECL"
printf '# The brief\n\nNothing restated here.\n' > "$F/docs/BRIEF.md"
git -C "$F" add -A && git -C "$F" -c user.name=t -c user.email=t@t commit -qm base
check "$F" > "$T/clean.out" || { cat "$T/clean.out"; fail "a clean fixture is refused"; }
rule="$(sentences "$F" | head -n 1)"
printf '\n%s\n' "$rule" >> "$F/docs/BRIEF.md"
git -C "$F" add -A && git -C "$F" -c user.name=t -c user.email=t@t commit -qm pasted
if check "$F" > "$T/pasted.out"; then fail "a rule pasted into docs/BRIEF.md is not refused"; fi
grep -q '^    docs/BRIEF.md carries a sentence of share/brief/report.yaml' "$T/pasted.out" \
  || { cat "$T/pasted.out"; fail "the refusal does not name docs/BRIEF.md"; }
exit 0
