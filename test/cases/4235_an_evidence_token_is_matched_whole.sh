# majordomus-covers: plan
# majordomus-negative: plan
# An evidence token is matched whole (I4235). `plan done` read the tokens an issue requires
# with a word-split loop, so a token that holds spaces was asked for word by word and an issue
# declaring "anthropic api adapter: request normalisation, credential, usage" could never be
# done. `plan evidence` wrote the token unquoted, and a ": " inside it is no plain YAML scalar.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

pj_init
pj_milestone M000
pj_issue I0001 M000
# the helper's one-word token, replaced by one with spaces, a colon and commas
TOKEN='an adapter: request normalisation, credential, usage'
F=.ai/repo/project/issues/I0001.yaml
sed "s/^  - proof\$/  - \"$TOKEN\"/" "$F" > "$F.new" && mv "$F.new" "$F"
grep -qxF "  - \"$TOKEN\"" "$F" || { echo "    the fixture did not take the token"; exit 1; }
"$MJ" plan start I0001 >/dev/null

# --- without its evidence the refusal names the whole token, not its words
expect_exit 10 "$MJ" plan "done" I0001
expect_grep "no evidence for: $TOKEN \(run:"

# --- recorded, the token is written quoted and read back whole
expect_exit 0 "$MJ" plan evidence I0001 --covers "$TOKEN" --type test --command true --result ok
grep -qxF "  - covers: \"$TOKEN\"" "$F" \
  || { echo "    the token was not written quoted:"; grep -n 'covers' "$F"; exit 1; }
expect_exit 0 "$MJ" plan validate
expect_exit 0 "$MJ" plan "done" I0001
expect_grep 'I0001 DONE'
