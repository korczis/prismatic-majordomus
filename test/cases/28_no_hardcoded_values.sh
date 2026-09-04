# Nothing the tool knows about itself may be written down twice.
#
# Every list in this case is derived — from the dispatch table, from the registries, from
# the generator's own declared inputs — so a new command, doctrine, claim or policy key
# joins these checks by existing, not by someone remembering to add it here. A case that
# enumerates what it checks is the same second source of truth it is trying to prevent.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- policy values
# A default written beside a reader is a second source of truth for the same number, and
# nothing keeps the two in step. Compare full key paths, never the last segment: checkpoint
# and handover both end in retention_max_files with different values, and comparing tails
# reports a drift between two keys that were each correct.
bad=0
for f in "$ROOT"/lib/*.sh; do
  case "$(basename "$f")" in common.sh) continue ;; esac        # defines mj_pol_req itself
  if grep -nE 'mj_pol +[a-z_.]+\)"; \[ -n' "$f" | grep -v '^[0-9]*: *#'; then
    echo "    $(basename "$f"): reads a policy value with its own default; use mj_pol_req"; bad=1
  fi
done
[ "$bad" = 0 ] || exit 1

# every literal policy key the code reads is declared in the skeleton policy. The whole
# dotted path is compared, never the last segment: checkpoint.retention_max_files and
# handover.retention_max_files are different keys with different values.
"$MJ" init >/dev/null
( . "$ROOT/lib/common.sh"; mj_yaml_flatten "$ROOT/share/skeleton/policy.yaml" ) > skeleton.flat
for k in $(grep -rhE 'mj_pol(_req)? +[a-z_]+(\.[a-z_]+)*' "$ROOT/lib" | grep -v '^[[:space:]]*#' \
           | grep -oE 'mj_pol(_req)? +[a-z_]+(\.[a-z_]+)*' | sed -E 's/mj_pol(_req)? +//' | sort -u); do
  grep -qx "$k=.*" skeleton.flat || grep -q "^$k\." skeleton.flat \
    || { echo "    lib/ reads policy key '$k' which share/skeleton/policy.yaml does not declare"; exit 1; }
done

# a missing required key is refused with the key named, not silently defaulted
"$MJ" update >/dev/null
sed -i.bak '/^  builder_budget_lines:/d' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak
expect_exit 10 "$MJ" context
expect_grep "missing required key 'context.builder_budget_lines'"
git checkout -q -- . 2>/dev/null || true
"$MJ" init --force >/dev/null; "$MJ" update >/dev/null

# ---------------------------------------------------------------- commands
# Derived from the dispatch table: usage, reference documentation and a behavioural case
# are required for every command, so a new one cannot arrive undocumented or untested.
COMMANDS="$(grep -oE '^  [a-z|]+\)$' "$ROOT/bin/majordomus" | tr -d ' )' | tr '|' '\n' | sort -u)"
[ "$(printf '%s\n' "$COMMANDS" | wc -w | tr -d ' ')" -ge 8 ] || { echo "    could not read the dispatch table"; exit 1; }
for c in $COMMANDS; do
  grep -qE "^  $c( |$)" "$ROOT/bin/majordomus" || { echo "    $c is dispatched but absent from usage"; exit 1; }
  grep -qE "^## \`majordomus $c" "$ROOT/docs/CLI.md" || { echo "    $c has no docs/CLI.md section"; exit 1; }
  grep -rqE "MJ\" (--json )?$c( |$)|MJ' $c " "$ROOT"/test/cases/*.sh || { echo "    no case invokes majordomus $c"; exit 1; }
done

# ---------------------------------------------------------------- doctrines
# Derived from the registry: every doctrine's validator exists, every command it names
# dispatches, and every test it names is a file. No list of doctrines appears here.
reg="$ROOT/share/doctrines.yaml"
[ -f "$reg" ] && {
  ids="$(grep -E '^  - id:' "$reg" | sed 's/^  - id: //')"
  [ -n "$ids" ] || { echo "    the doctrine registry declares nothing"; exit 1; }
  n=0
  for id in $ids; do
    n=$((n + 1))
    val="$(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    validator:/{print $2; exit}' "$reg")"
    tst="$(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    test:/{print $2; exit}' "$reg")"
    cls="$(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    class:/{print $2; exit}' "$reg")"
    grep -rqE "^mj_validate_$val\(\)" "$ROOT/lib" || { echo "    doctrine $id names validator $val, which no lib/ file defines"; exit 1; }
    [ -f "$ROOT/$tst" ] || { echo "    doctrine $id names test $tst, which does not exist"; exit 1; }
    case "$cls" in blocking|advisory) ;; *) echo "    doctrine $id has class '$cls'"; exit 1 ;; esac
    for cmd in $(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    enforced_by:/{gsub(/[][,]/," ");sub(/enforced_by:/,"");print;exit}' "$reg"); do
      grep -q 'mj_doctrine_dispatch' "$ROOT/lib/$cmd.sh" 2>/dev/null \
        || { echo "    doctrine $id is enforced_by '$cmd', which does not dispatch"; exit 1; }
    done
  done
  # and no validator exists that no doctrine declares
  for fn in $(grep -rhoE '^mj_validate_[a-z_]+\(\)' "$ROOT/lib" | sed -e 's/^mj_validate_//' -e 's/()//' | sort -u); do
    grep -qE "^    validator: $fn$" "$reg" || { echo "    lib/ defines mj_validate_$fn, which no doctrine declares"; exit 1; }
  done
}

# ---------------------------------------------------------------- claims
# Derived from the matrix: every claim has a detail page, every guaranteed one has a test
# that exists, and every path it names resolves.
claim_ids() { awk '/^claims:/{c=1;next} c&&/^  - id: /{print $3}' "$ROOT/docs/CLAIMS.yaml"; }
for id in $(claim_ids); do
  [ -f "$ROOT/docs/claims/$id.md" ] || { echo "    claim $id has no docs/claims/$id.md"; exit 1; }
  for sec in 'What it means' 'How it works' 'How to see it' 'What it does not cover' 'Why it'; do
    grep -q "^## $sec" "$ROOT/docs/claims/$id.md" || { echo "    docs/claims/$id.md lacks the '$sec' section"; exit 1; }
  done
  st="$(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    status:/{print $2; exit}' "$ROOT/docs/CLAIMS.yaml")"
  tst="$(awk -v i="  - id: $id" '$0==i{f=1} f&&/^    test:/{print $2; exit}' "$ROOT/docs/CLAIMS.yaml" | tr -d "'")"
  case "$st" in
    guaranteed) [ "$tst" != "-" ] && [ -f "$ROOT/$tst" ] || { echo "    guaranteed claim $id names test '$tst', which does not exist"; exit 1; } ;;
    advisory|planned|rejected) ;;
    *) echo "    claim $id has status '$st'"; exit 1 ;;
  esac
done


# ---------------------------------------------------------------- claim statuses
# The set of statuses a claim may carry is declared once, in the matrix, and every reader
# derives it. It used to appear in a comment, in a JSON literal inside the generator, in
# two loops beside it, and in site-check — five copies of one closed vocabulary.
grep -q '^statuses:' "$ROOT/docs/CLAIMS.yaml" || { echo "    docs/CLAIMS.yaml does not declare its statuses as data"; exit 1; }
for f in "$ROOT/scripts/generate-site-data" "$ROOT/scripts/site-check"; do
  grep -qE 'in guaranteed advisory planned rejected' "$f" \
    && { echo "    $(basename "$f") lists the claim statuses instead of deriving them"; exit 1; }
done
# every status a claim actually uses is one the matrix declares
for st in $(grep -E '^    status:' "$ROOT/docs/CLAIMS.yaml" | sed 's/^    status: //' | sort -u); do
  awk '/^statuses:/{c=1;next} /^claims:/{c=0} c&&/^  - id: /{print $3}' "$ROOT/docs/CLAIMS.yaml" \
    | grep -qx "$st" || { echo "    a claim uses status '$st', which the matrix does not declare"; exit 1; }
done

# ---------------------------------------------------------------- vocabulary
# Every bold term in the concepts table is a word the tool actually uses: it appears in the
# CLI reference, in a schema, or in the source. A vocabulary entry nothing implements is
# the documentation drifting away from the product.
for term in $(grep -oE '^\| \*\*[a-z ]+\*\*' "$ROOT/docs/CONCEPTS.md" | sed -e 's/^| \*\*//' -e 's/\*\*$//' -e 's/ /_/g'); do
  word="$(printf '%s' "$term" | tr '_' ' ')"
  # --exclude must precede the paths: after them, BSD grep treats it as a filename, the
  # term matches itself in CONCEPTS.md, and the whole check passes vacuously
  grep -rqi --exclude=CONCEPTS.md -- "$word" \
       "$ROOT/docs" "$ROOT/lib" "$ROOT/bin/majordomus" "$ROOT/share" \
    || { echo "    concepts table defines '$word', which appears nowhere else in the repository"; exit 1; }
done

# ---------------------------------------------------------------- generator inputs
# The generator declares its own inputs and the fixtures derive from that declaration, so
# neither can go stale against the other. Six fixtures once carried their own copy list and
# all six broke at once when a new canonical input was added.
"$ROOT/scripts/generate-site-data" --inputs > inputs.txt
[ -s inputs.txt ] || { echo "    generate-site-data --inputs printed nothing"; exit 1; }
while IFS= read -r p; do
  [ -f "$ROOT/$p" ] || { echo "    generator declares input $p, which does not exist"; exit 1; }
done < inputs.txt
grep -q 'fixture_repo' "$ROOT/test/lib.sh" || { echo "    test/lib.sh has no derived fixture builder"; exit 1; }
# The rule is about canonical inputs, not about copying the runtime. A fixture that names a
# canonical input in its own copy list is the pair of lists that went stale; a fixture that
# copies bin/lib/share/scripts and then builds its own model is not, and 47_mutation does
# exactly that on purpose.
for f in "$ROOT"/test/cases/*.sh; do
  grep -q 'generate-site-data' "$f" || continue
  # the top-level directory of each canonical input, minus the runtime a fixture always copies
  for p in $("$ROOT/scripts/generate-site-data" --inputs | grep '/' | cut -d/ -f1 | sort -u); do
    case "$p" in bin|lib|share|scripts|docs) continue ;; esac
    if grep -qF "\$ROOT/$p" "$f" && ! grep -q 'fixture_repo' "$f"; then
      echo "    $(basename "$f") copies canonical input tree $p by hand; use fixture_repo"; exit 1
    fi
  done
done

# The vocabulary check above must actually be able to fail. A term nothing implements is
# introduced into a copy of the table and the same test must reject it.
cp "$ROOT/docs/CONCEPTS.md" concepts.probe
printf '| **quantum entanglement** | a term nothing implements | nowhere |\n' >> concepts.probe
found=0
for term in $(grep -oE '^\| \*\*[a-z ]+\*\*' concepts.probe | sed -e 's/^| \*\*//' -e 's/\*\*$//' -e 's/ /_/g'); do
  word="$(printf '%s' "$term" | tr '_' ' ')"
  grep -rqi --exclude=CONCEPTS.md -- "$word" \
       "$ROOT/docs" "$ROOT/lib" "$ROOT/bin/majordomus" "$ROOT/share" || found=1
done
[ "$found" = 1 ] || { echo "    the vocabulary check cannot fail; it is vacuous"; exit 1; }

# ---------------------------------------------------------------- empty is not a result
# The sharper half of the same rule: a derivation that produces nothing is a fault in the
# derivation, not an answer. Every instance found so far returned something plausible
# instead of stopping — a count of the wrong shape, a regex that matched nothing by
# construction, a runner reporting "0 passed" for a case that did not exist.
#
# Each list this case derives is checked for being non-empty, so a scan that silently stops
# matching turns this case red instead of passing vacuously.
[ -n "$COMMANDS" ] || { echo "    the command list derived from the dispatch table is empty"; exit 1; }
[ -s inputs.txt ] || { echo "    the generator declared no inputs"; exit 1; }
[ -n "$(claim_ids)" ] || { echo "    the claims section derived no ids"; exit 1; }
[ -n "$(grep -oE '^\| \*\*[a-z ]+\*\*' "$ROOT/docs/CONCEPTS.md")" ] \
  || { echo "    the concepts table derived no terms"; exit 1; }
if [ -f "$ROOT/share/doctrines.yaml" ]; then
  [ -n "$(grep -E '^  - id:' "$ROOT/share/doctrines.yaml")" ] || { echo "    the doctrine registry derived no ids"; exit 1; }
fi

# ---------------------------------------------------------------- locale independence
# The generator hashes its input list, so the order of that list is part of the output. A
# glob expands in the collation order of whoever runs it, and a directory holding both
# M000.yaml and slug-named files sorts differently under byte order than under a
# case-insensitive locale. That made the committed hash differ between a developer's
# machine and CI, and CI was right — the tool declares byte-identical output for identical
# input, and "identical input" cannot mean "on the same machine".
a="$(LC_ALL=C "$ROOT/scripts/generate-site-data" --inputs)"
b="$(LC_ALL=en_US.UTF-8 "$ROOT/scripts/generate-site-data" --inputs 2>/dev/null || printf '%s' "$a")"
[ "$a" = "$b" ] || { echo "    the generator's input order depends on the locale; it must expand its globs in byte order"; exit 1; }
