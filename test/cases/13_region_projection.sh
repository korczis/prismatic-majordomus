# majordomus-covers: update
# majordomus-negative: update doctor watch
. "$ROOT/test/lib.sh"
# A region projection owns only the text between its markers. Everything else in the
# host document is the repository's, and must survive update, doctor and watch untouched.
"$MJ" init >/dev/null

# a repository that already has a hand-written CLAUDE.md
cat > CLAUDE.md <<'MD'
# CLAUDE.md

Hand-written governance that predates Majordomus. Nothing here is generated.

## A rule the repository already had

Keep it.
MD
before_sha="$(shasum -a 256 CLAUDE.md | cut -d' ' -f1)"

# one projection, region mode
awk '/^projections:/{exit} {print}' .majordomus/policy.yaml > policy.new
cat >> policy.new <<'YAML'
projections:
  - provider: claude-code
    target: CLAUDE.md
    mode: region
    always_loaded: true
YAML
mv policy.new .majordomus/policy.yaml

# an unknown mode is refused rather than guessed at
sed 's/    mode: region/    mode: nonsense/' .majordomus/policy.yaml > p.tmp && mv p.tmp .majordomus/policy.yaml
expect_exit 10 "$MJ" update
expect_grep "unknown mode 'nonsense'"
expect_grep 'Hand-written governance' CLAUDE.md
sed 's/    mode: nonsense/    mode: region/' .majordomus/policy.yaml > p.tmp && mv p.tmp .majordomus/policy.yaml

# first update appends the region and leaves the rest of the document alone
expect_exit 0 "$MJ" update
expect_grep '^create CLAUDE.md$'
expect_grep '^<!-- majordomus:begin [0-9a-f]{12} -->$' CLAUDE.md
expect_grep '^<!-- majordomus:end -->$' CLAUDE.md
expect_grep 'Hand-written governance that predates Majordomus' CLAUDE.md
expect_grep 'A rule the repository already had' CLAUDE.md
# the fingerprint records the region, not the file
expect_no_grep "sha256: $before_sha" .majordomus/generated/fingerprints.yaml
expect_grep 'mode: region' .majordomus/generated/fingerprints.yaml

# deterministic: a second update changes nothing
after="$(shasum -a 256 CLAUDE.md | cut -d' ' -f1)"
expect_exit 0 "$MJ" update
expect_grep '^unchanged CLAUDE.md$'
[ "$(shasum -a 256 CLAUDE.md | cut -d' ' -f1)" = "$after" ] || { echo "    update was not idempotent"; exit 1; }
expect_exit 0 "$MJ" watch
expect_grep 'OK   projection +CLAUDE.md — region matches fingerprint'

# THE POINT: editing outside the region is the repository's business, not drift
printf '\n## A rule added after adoption\n\nAlso keep it.\n' >> CLAUDE.md
expect_exit 0 "$MJ" watch
expect_grep 'region matches fingerprint'
expect_exit 0 "$MJ" update
expect_grep '^unchanged CLAUDE.md$'
expect_grep 'A rule added after adoption' CLAUDE.md

# editing inside the region is detected, and update refuses to overwrite it silently.
# The edit targets the region's own first body line rather than a phrase from the projected
# prose, so rewording the provider body cannot silently turn this case into a no-op.
awk '/^<!-- majordomus:begin/{print; getline; print "EDITED " $0; next} {print}' CLAUDE.md > c.tmp && mv c.tmp CLAUDE.md
expect_grep 'EDITED' CLAUDE.md
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT projection +CLAUDE.md — region differs from fingerprint'
expect_exit 15 "$MJ" update
expect_grep 'REFUSE projection +CLAUDE.md'
expect_grep 'EDITED' CLAUDE.md
# --diff shows the region, not the whole file
expect_exit 0 "$MJ" update --diff CLAUDE.md
expect_no_grep 'Hand-written governance'
expect_exit 0 "$MJ" update --force
expect_no_grep 'EDITED' CLAUDE.md
expect_grep 'Hand-written governance' CLAUDE.md
expect_grep 'A rule added after adoption' CLAUDE.md

# malformed markers are refused, never guessed at
grep -v '^<!-- majordomus:end -->$' CLAUDE.md > c.tmp && mv c.tmp CLAUDE.md
expect_exit 15 "$MJ" update
expect_grep 'region markers are malformed'
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL projection +CLAUDE.md — region markers are malformed'

# the budget measures the generated region, and over budget nothing is written at all
"$MJ" init --force >/dev/null
cat > CLAUDE.md <<'MD'
# CLAUDE.md

Hand-written.
MD
awk '/^projections:/{exit} {print}' .majordomus/policy.yaml > policy.new
cat >> policy.new <<'YAML'
projections:
  - provider: claude-code
    target: CLAUDE.md
    mode: region
    always_loaded: true
YAML
mv policy.new .majordomus/policy.yaml
sed 's/^  always_loaded_budget_lines: .*/  always_loaded_budget_lines: 5/' .majordomus/policy.yaml > p.tmp && mv p.tmp .majordomus/policy.yaml
expect_exit 10 "$MJ" update
expect_grep 'FAIL budget +CLAUDE.md — would be [0-9]+ lines, budget 5; nothing written'
expect_no_grep 'majordomus:begin' CLAUDE.md
