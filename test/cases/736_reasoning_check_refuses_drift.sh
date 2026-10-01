# majordomus-negative
# claims: review-is-never-fabricated
# What the reasoning design forbids is detected, not merely discouraged (ADR 0098). Each
# defect is planted in a fixture and `reasoning check` must name it and exit 10, and
# `doctor` must fail on it while reporting absent advisors as information only:
# provider-independent code naming an advisor, an adapter the catalogue names and nothing
# implements, a reference to a vendor nobody declares, CI naming a model credential, a
# document claiming consensus guarantees correctness or that an advisor is required, a
# consultation record claiming an advisor its plan never selected, and a conclusion whose
# review count does not match what it cites. A secret typed into a record never reaches
# the store.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "no jq"
reasoning_fixture

expect_exit 0 rz reasoning check
expect_grep "no finding"

plant() { expect_exit 10 rz reasoning check; expect_grep "FAIL $1"; expect_grep "$2"; }

# ---------------------------------------------------------------- coupling
cp "$R/scripts/lib/advisors/driver.mjs" "$T/driver.bak"
printf "\nconst preferred = 'codex';\n" >> "$R/scripts/lib/advisors/driver.mjs"
plant "coupling" "provider-independent code names 'codex'"
cp "$T/driver.bak" "$R/scripts/lib/advisors/driver.mjs"

# ---------------------------------------------------------------- adapters and references
mv "$R/scripts/lib/advisors/gemini-cli.mjs" "$T/"
plant "adapters" "adapter 'gemini-cli' and no transport module"
mv "$T/gemini-cli.mjs" "$R/scripts/lib/advisors/"
cp "$R/share/advisors.yaml" "$T/advisors.bak"
sed 's/vendor: ollama/vendor: nowhere/' "$T/advisors.bak" > "$R/share/advisors.yaml"
plant "catalogue" "names vendor 'nowhere'"
cp "$T/advisors.bak" "$R/share/advisors.yaml"

# ---------------------------------------------------------------- CI never needs a live model
mkdir -p "$R/.github/workflows"
printf 'jobs:\n  review:\n    env:\n      KEY: ${{ secrets.OPENAI_API_KEY }}\n' > "$R/.github/workflows/review.yml"
plant "ci" "CI names the model credential OPENAI_API_KEY"
rm -r "$R/.github"

# ---------------------------------------------------------------- claim drift
mkdir -p "$R/docs"
printf '# Review\n\nModel consensus guarantees correctness.\n' > "$R/docs/REVIEW.md"
plant "claims" "consensus guarantees"
printf '# Review\n\nThis workflow requires Gemini CLI.\n' > "$R/docs/REVIEW.md"
plant "claims" "requires gemini cli"
rm "$R/docs/REVIEW.md"

# ---------------------------------------------------------------- fabricated review
A="$(rz_record '{"kind":"assessment","subject":"s","materiality":"material","evidence":[{"kind":"file","reference":"f"}]}')"
P="$(rz_record "{\"kind\":\"plan\",\"assessment\":\"$A\"}")"
dir="$(dirname "$(ls "$R"/.ai/local/state/reasoning/*/"$P".json)")"
# written past the writer, as a careless or dishonest session would
jq --arg p "$P" '.id = "consultation-forged" | .body = {"kind":"consultation","plan":$p,"advisor":"gemini","status":"completed","conclusion":"looks fine","stance":"supports"}' \
  "$dir/$P.json" > "$dir/consultation-forged.json"
plant "records" "claims advisor 'gemini', which plan $P did not select"
jq '.body.advisor = "someone-else"' "$dir/consultation-forged.json" > "$dir/x" && mv "$dir/x" "$dir/consultation-forged.json"
plant "records" "names advisor 'someone-else', which the catalogue does not declare"
rm "$dir/consultation-forged.json"
K="$(rz_record "{\"kind\":\"conclusion\",\"assessment\":\"$A\",\"decision\":\"d\",\"rationale\":\"r\",\"evidence\":[{\"kind\":\"file\",\"reference\":\"f\"}],\"validation_plan\":[\"v\"]}")"
jq '.body.reviewed_by = ["chatgpt"] | .body.independent_review_count = 1' "$dir/$K.json" > "$dir/x" && mv "$dir/x" "$dir/$K.json"
plant "records" "claims review by \[chatgpt\] \(1\), and the consultations it cites show \[\]"

# doctor fails on the finding, and reports every absent advisor as information
( cd "$R" && rz_env MAJORDOMUS_BIN="$RB" "$R/bin/majordomus" doctor > "$T/doctor.txt" 2>&1 ) || true
grep -q "^FAIL reasoning .*records: claims review by" "$T/doctor.txt" || { echo "    doctor did not fail on the forged review"; grep -i reason "$T/doctor.txt"; exit 1; }
grep -q "^INFO advisor     codex — unavailable (executable_not_found) — optional" "$T/doctor.txt" \
  || { echo "    doctor does not report an absent advisor as information"; grep -i advisor "$T/doctor.txt"; exit 1; }
if grep -qE "^(FAIL|WARN) advisor" "$T/doctor.txt"; then echo "    doctor treats an absent advisor as a problem"; exit 1; fi
rm "$dir/$K.json"

# ---------------------------------------------------------------- secrets never reach the store
key="sk-ant-$(printf 'b%.0s' $(seq 1 24))"
S="$(rz_record "{\"kind\":\"assessment\",\"subject\":\"the key $key leaked into a log\",\"materiality\":\"low\"}")"
f="$dir/$S.json"
grep -q "sk-ant-" "$f" && { echo "    a secret reached the store"; exit 1; }
[ "$(jq -r '.redacted[0]' "$f")" = anthropic-key ] || { echo "    the redaction is not recorded"; exit 1; }
expect_exit 0 rz reasoning check
