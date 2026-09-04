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
sed -i.bak '/^  builder_budget_lines:/d' .ai/repo/policy.yaml; rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$MJ" context
# the key is named; how the message punctuates it is not the contract, and asserting the
# quotes is what made this case fail when the reporting changed and the behaviour did not
expect_grep "missing required key.*context\\.builder_budget_lines"
reset_policy; "$MJ" update >/dev/null

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
# Derived from the standard rule package: every enforced rule's validator exists, every
# command it names dispatches, and every test it names is a file. No list of rules
# appears here; the package manifest is read, and the package is the one the tool vendors.
PKG="$ROOT/share/standard/majordomus"
[ -f "$PKG/manifest.yaml" ] || { echo "    the distribution ships no standard rule package"; exit 1; }
rule_files="$(sed -n 's/^    file: //p' "$PKG/manifest.yaml")"
[ -n "$rule_files" ] || { echo "    the package manifest lists no rules"; exit 1; }
declared=" "
for rf in $rule_files; do
  f="$PKG/$rf"; [ -f "$f" ] || { echo "    the manifest lists $rf, which does not exist"; exit 1; }
  front="$(awk 'NR==1&&$0!="---"{exit 2} NR>1&&$0=="---"{exit} NR>1' "$f")"
  id="$(printf '%s\n' "$front" | sed -n 's/^id: //p')"
  cls="$(printf '%s\n' "$front" | sed -n 's/^class: //p')"
  case "$cls" in blocking|advisory) ;; *) echo "    rule $id has class '$cls'"; exit 1 ;; esac
  val="$(printf '%s\n' "$front" | sed -n 's/^  validator: //p')"
  [ -n "$val" ] || continue   # a principle: normative, enforced by nobody, and says so
  declared="$declared$val "
  grep -rqE "^mj_validate_$val\(\)" "$ROOT/lib" || { echo "    rule $id names validator $val, which no lib/ file defines"; exit 1; }
  for tst in $(printf '%s\n' "$front" | sed -n 's/^  tests: \[\(.*\)\]/\1/p' | tr ',' ' '); do
    [ -f "$ROOT/$tst" ] || { echo "    rule $id names test $tst, which does not exist"; exit 1; }
  done
  for cmd in $(printf '%s\n' "$front" | sed -n 's/^  enforced_by: \[\(.*\)\]/\1/p' | tr ',' ' '); do
    grep -q 'mj_doctrine_dispatch' "$ROOT/lib/$cmd.sh" 2>/dev/null \
      || { echo "    rule $id is enforced_by '$cmd', which does not dispatch"; exit 1; }
  done
done
[ "$declared" != " " ] || { echo "    no rule in the package names a validator"; exit 1; }
# and no validator exists that no rule declares
for fn in $(grep -rhoE '^mj_validate_[a-z_]+\(\)' "$ROOT/lib" | sed -e 's/^mj_validate_//' -e 's/()//' | sort -u); do
  case "$declared" in *" $fn "*) ;; *) echo "    lib/ defines mj_validate_$fn, which no rule declares"; exit 1 ;; esac
done

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

# ---------------------------------------------------------------- responsibilities
# The README "What it does" table is the prose; docs/RESPONSIBILITIES.yaml is the machine
# side of the same fact. They are joined by row title, and a join is silent when it misses:
# an entry whose readme_key no longer matches renders a page with no command and no claims,
# which is indistinguishable from a responsibility that genuinely has none. So the
# correspondence is asserted in both directions, and every path and heading an entry names
# is resolved. Nothing here enumerates a responsibility; the list is read from the file.
resp_field() { awk -v i="  - id: $1" -v k="    $2:" '$0==i{f=1;next} f&&/^  - id: /{exit} f&&index($0,k)==1{sub(k" *","");print;exit}' "$ROOT/docs/RESPONSIBILITIES.yaml"; }
resp_ids="$(awk '/^responsibilities:/{c=1;next} c&&/^  - id: /{print $3}' "$ROOT/docs/RESPONSIBILITIES.yaml")"
[ -n "$resp_ids" ] || { echo "    docs/RESPONSIBILITIES.yaml declared no responsibilities"; exit 1; }
readme_titles="$(awk '/^## What it does/{f=1;next} f&&/^## /{exit} f' "$ROOT/README.md" \
                 | grep -E '^\| \*\*' | sed -E 's/^\| \*\*([^*]+)\*\*.*/\1/')"
[ -n "$readme_titles" ] || { echo "    the README 'What it does' table produced no rows"; exit 1; }
for id in $resp_ids; do
  rk="$(resp_field "$id" readme_key)"
  printf '%s\n' "$readme_titles" | grep -qxF "$rk" \
    || { echo "    responsibility $id names README row '$rk', which the table does not have"; exit 1; }
  # files: is a flow list; implementation: is a single path. Both must resolve.
  for p in $(resp_field "$id" files | tr -d '[],') $(resp_field "$id" implementation); do
    case "$p" in .ai/local/*) continue ;; esac   # checkout-local; absent on a fresh clone by design
    [ -e "$ROOT/$p" ] || { echo "    responsibility $id names missing path $p"; exit 1; }
  done
  for pair in "cli_anchor docs/CLI.md" "schema_anchor docs/SCHEMAS.md"; do
    fld="${pair% *}"; doc="${pair#* }"; val="$(resp_field "$id" "$fld")"
    [ -n "$val" ] || { echo "    responsibility $id has an empty $fld"; exit 1; }
    grep -qxF "## $val" "$ROOT/$doc" || grep -qxF "## \`$val\`" "$ROOT/$doc" \
      || { echo "    responsibility $id names $fld '$val', which is not a heading in $doc"; exit 1; }
  done
done
# the other direction: a README row with no entry is the same silence seen from the far side.
# Read line by line rather than word-splitting: a row title is free text and may hold spaces.
while IFS= read -r t; do
  [ -n "$t" ] || continue
  found=0
  for id in $resp_ids; do [ "$(resp_field "$id" readme_key)" = "$t" ] && { found=1; break; }; done
  [ "$found" = 1 ] || { echo "    README row '$t' has no entry in docs/RESPONSIBILITIES.yaml"; exit 1; }
done <<EOF
$readme_titles
EOF
# and the generator must not have reintroduced a default for the missing case: a fallback
# there restores exactly the silence these checks exist to remove
grep -q 'r_by_key\[\$row.title\] // {}' "$ROOT/scripts/generate-site-data" \
  && { echo "    generate-site-data defaults a missing responsibility instead of refusing"; exit 1; }

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
[ -n "$(sed -n 's/^  - id: //p' "$ROOT/share/standard/majordomus/manifest.yaml")" ] || { echo "    the rule package manifest derived no ids"; exit 1; }

# ---------------------------------------------------------------- argv limits
# The same shape of fault as the locale check below: correct on the machine everyone develops
# on, broken on every runner. Linux caps a SINGLE argv entry at MAX_ARG_STRLEN, 131072 bytes,
# independently of the much larger total ARG_MAX; execve then fails with E2BIG and the shell
# reports exit 126. macOS has no per-argument cap of that shape. A model collection passed as
# one --argjson argument therefore grows quietly until a deploy goes red, which is what
# happened once the plan reached sixty-odd issue contracts.
#
# The margin is measured against the real model rather than a list of field names, so a
# collection that grows into the danger zone joins this check by growing, not by being added
# here. --slurpfile reads a file and has no ceiling at all.
plan="$ROOT/site/data/generated/plan.json"
if [ -f "$plan" ]; then
  arrays="$(jq -r 'to_entries[] | select(.value | type == "array") | .key' "$plan")"
  [ -n "$arrays" ] || { echo "    plan.json declares no collections; the argv check is vacuous"; exit 1; }
  for k in $arrays; do
    n="$(jq -c --arg k "$k" '.[$k]' "$plan" | wc -c | tr -d ' ')"
    [ "$n" -lt 100000 ] && continue
    grep -qE -- "--slurpfile $k " "$ROOT/scripts/generate-site-data" \
      || { echo "    plan.json .$k is $n bytes and reaches jq through argv; Linux refuses one argument over 131072"; exit 1; }
  done
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
