# majordomus-covers: capture
# majordomus-negative: capture doctor
# The prompt archive: joinable after the episode closes, private, bounded, and free of
# credential material.
#
# Every assertion here goes through the provider's own shims under .claude/hooks/ and not
# through the library, because the whole subsystem exists below the model and a lifecycle
# that only works when a test calls a shell function proves nothing about the hook the
# provider actually runs. The episode is opened and closed by the session shims; the prompts
# arrive through the capture shim; the reconciliation and the pruning are the commands a
# person runs afterwards.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
echo a > a && git add -A && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH
"$MJ" capture install >/dev/null
P=.ai/local/prompts
records() { find "$P" -maxdepth 1 -name '*.json' | wc -l | tr -d ' '; }
field() { sed -n "s/^  \"$2\": \\(.*\\)\$/\\1/p" "$1" | sed 's/,$//' | head -n 1; }
say() { printf '%s' "$1" | ./.claude/hooks/majordomus-capture; }
rec_for() { find "$P" -maxdepth 1 -name "*-$1.json" | head -n 1; }

# ---------------------------------------------------------------- the join, while it is easy
# A prompt that arrives inside an open episode names that episode, and says it observed it
# rather than inferred it.
printf '{"session_id":"uuid-alpha","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
alpha="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/uuid-alpha.yaml | head -n 1)"
[ -n "$alpha" ] || { echo "    the start shim opened no episode"; ls -1 .ai/local/state/sessions-open 2>/dev/null; exit 1; }
say '{"session_id":"uuid-alpha","prompt_id":"a1","prompt_source":"user","prompt_text":"inside the episode"}'
r="$(rec_for inside-the-episode)"
[ -n "$r" ] || { echo "    the prompt inside an open episode was not captured"; ls -1 "$P"; exit 1; }
[ "$(field "$r" episode)" = "\"$alpha\"" ] \
  || { echo "    the record does not name the open episode"; sed 's/^/    | /' "$r"; exit 1; }
[ "$(field "$r" episode_link)" = '"open"' ] \
  || { echo "    a link resolved against an open episode is not recorded as observed"; sed 's/^/    | /' "$r"; exit 1; }
# and the identity the rest of the tool computes, not a second notion of it
[ "$(field "$r" worktree_id)" != null ] && [ "$(field "$r" repository_id)" != null ] \
  || { echo "    the record carries no repository or worktree identity"; sed 's/^/    | /' "$r"; exit 1; }

# ---------------------------------------------------------------- never somebody else's episode
# A second episode is open in the same checkout. A prompt naming a provider session with no
# episode of its own must not be handed either of them: the pointer names whichever opened
# last, and attaching a prompt to it would be quietly false rather than visibly missing.
printf '{"session_id":"uuid-beta","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
beta="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/uuid-beta.yaml | head -n 1)"
[ -n "$beta" ] && [ "$beta" != "$alpha" ] || { echo "    the second episode did not open separately"; exit 1; }
say '{"session_id":"uuid-nobody","prompt_id":"o1","prompt_source":"user","prompt_text":"an unknown provider session"}'
r="$(rec_for an-unknown-provider-session)"
[ "$(field "$r" episode)" = null ] \
  || { echo "    a prompt with no episode of its own was given one"; sed 's/^/    | /' "$r"; exit 1; }
[ "$(field "$r" episode_link)" = '"orphan"' ] \
  || { echo "    an unresolvable capture was not recorded as an orphan"; sed 's/^/    | /' "$r"; exit 1; }
# a payload that names no session at all is the same answer, and not a crash
say '{"prompt_id":"o2","prompt_source":"user","prompt_text":"no session at all"}'
r="$(rec_for no-session-at-all)"
[ "$(field "$r" episode_link)" = '"orphan"' ] \
  || { echo "    a payload with no session was not recorded as an orphan"; sed 's/^/    | /' "$r"; exit 1; }

# ---------------------------------------------------------------- the join survives the close
# This is the defect the whole case is about. The end shim deletes state/sessions-open/<uuid>,
# which was the only mapping from the provider's session to the episode. A record written
# before this point keeps its episode because it carries it; a record written before the field
# existed has to be reconciled from what survived.
printf '{"schema":"majordomus.capture/v1","ts":"20010101000000","provider":"claude-code","event":"UserPromptSubmit","id":"L1","session":"uuid-alpha","source":null,"cwd":null,"repository":"r","branch":"b","head":"h","text":"a legacy prompt of alpha"}\n' \
  > "$P/20010101000000-legacy-alpha.json"
printf '{"schema":"majordomus.capture/v1","ts":"20010101000001","provider":"claude-code","event":"UserPromptSubmit","id":"L2","session":"uuid-gone","source":null,"cwd":null,"repository":"r","branch":"b","head":"h","text":"a legacy prompt of nobody"}\n' \
  > "$P/20010101000001-legacy-gone.json"
printf '{"session_id":"uuid-alpha","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1
[ ! -f .ai/local/state/sessions-open/uuid-alpha.yaml ] \
  || { echo "    the end shim left the open-episode file, so the case is not testing the defect"; exit 1; }
# the record written while it was open still knows
r="$(rec_for inside-the-episode)"
[ "$(field "$r" episode)" = "\"$alpha\"" ] \
  || { echo "    closing the episode changed what an already-written record says"; sed 's/^/    | /' "$r"; exit 1; }

expect_exit 0 "$MJ" capture reconcile --dry-run
expect_grep 'would be linked'
# --dry-run wrote nothing
expect_no_grep '"episode"' "$P/20010101000000-legacy-alpha.json"
expect_exit 0 "$MJ" capture reconcile
expect_grep 'unlinked for want of evidence'
# the session context of the closed episode is what survived, and it is named as the evidence
[ "$(field "$P/20010101000000-legacy-alpha.json" episode)" = "\"$alpha\"" ] \
  || { echo "    a legacy record was not linked from the surviving session context"; sed 's/^/    | /' "$P/20010101000000-legacy-alpha.json"; exit 1; }
[ "$(field "$P/20010101000000-legacy-alpha.json" episode_link)" = '"session-context"' ] \
  || { echo "    the provenance of an inferred link is not recorded as an inference"; exit 1; }
# ...and the one nothing can place is marked, not guessed at
[ "$(field "$P/20010101000001-legacy-gone.json" episode)" = null ] \
  || { echo "    a record with no evidence was given an episode anyway"; sed 's/^/    | /' "$P/20010101000001-legacy-gone.json"; exit 1; }
[ "$(field "$P/20010101000001-legacy-gone.json" episode_link)" = '"unlinked-legacy"' ] \
  || { echo "    a record with no evidence is not marked as an unlinked legacy prompt"; exit 1; }
# an orphan the writer recorded stays an orphan: it is a different fact from a legacy record,
# and collapsing the two would make a migration look like a live defect forever
[ "$(field "$(rec_for an-unknown-provider-session)" episode_link)" = '"orphan"' ] \
  || { echo "    reconcile relabelled an orphan as a legacy record"; exit 1; }
# reconcile is idempotent, and never re-decides a link it already has evidence for
expect_exit 0 "$MJ" capture reconcile
expect_grep '2 already linked|[0-9]+ already linked'
[ "$(field "$P/20010101000000-legacy-alpha.json" episode_link)" = '"session-context"' ] \
  || { echo "    a second reconcile changed an established provenance"; exit 1; }
# every record renders again, so the pair invariant survives the rewrite
[ -f "$P/20010101000000-legacy-alpha.md" ] && [ -f "$P/20010101000000-legacy-alpha.yaml" ] \
  || { echo "    reconcile left a record without its renderings"; ls -1 "$P"; exit 1; }

# ---------------------------------------------------------------- a credential never lands
# Through the real shim, with a payload shaped like the ones people actually send. The
# assertion is about what is on disk, not about what a function returned: the record, both
# renderings, the file name, and the diagnostic log are all searched for the secret.
secret='sk-ant-api03-NOTAREALKEYbutlongenough1234'
say "{\"session_id\":\"uuid-beta\",\"prompt_id\":\"s1\",\"prompt_source\":\"user\",\"prompt_text\":\"why does $secret return 401\"}"
if grep -rqF "$secret" "$P" 2>/dev/null; then
  echo "    the credential is on disk in the archive:"; grep -rlF "$secret" "$P" | sed 's/^/    | /'; exit 1
fi
if find "$P" -maxdepth 1 -name "*$secret*" | grep -q .; then
  echo "    the credential is in a file name"; exit 1
fi
r="$(find "$P" -maxdepth 1 -name '*-why-does-redacted*.json' | head -n 1)"
[ -n "$r" ] || { echo "    the redacted prompt was not captured at all"; ls -1 "$P"; exit 1; }
grep -qF '[redacted:anthropic-key]' "$r" || { echo "    the credential was not replaced by a named marker"; sed 's/^/    | /' "$r"; exit 1; }
[ "$(field "$r" redacted)" = '"anthropic-key"' ] \
  || { echo "    the record does not say which credential shape was replaced"; sed 's/^/    | /' "$r"; exit 1; }
grep -qF '[redacted:anthropic-key]' "${r%.json}.md" || { echo "    the Markdown rendering is not redacted"; exit 1; }
grep -qF '[redacted:anthropic-key]' "${r%.json}.yaml" || { echo "    the YAML rendering is not redacted"; exit 1; }
# a record that carried no credential says nothing about redaction: absent is "none found",
# and a present-but-empty field would be a claim nobody can support
expect_no_grep '"redacted"' "$(rec_for inside-the-episode)"
# an assignment keeps its left-hand side, because the name is what makes the prompt readable
say '{"session_id":"uuid-beta","prompt_id":"s2","prompt_source":"user","prompt_text":"export API_KEY=abcdefghijklmnopqrstuvwxyz please"}'
r="$(find "$P" -maxdepth 1 -name '*export-api-key*.json' | head -n 1)"
[ -n "$r" ] || { echo "    the assignment prompt was not captured"; ls -1 "$P"; exit 1; }
grep -qF 'export API_KEY=[redacted:assignment]' "$r" \
  || { echo "    an assignment was not redacted, or took the name with it"; sed 's/^/    | /' "$r"; exit 1; }
# ...and ordinary prose that merely talks about secrets is not mangled
say '{"session_id":"uuid-beta","prompt_id":"s3","prompt_source":"user","prompt_text":"why does the archive keep secret things, and what is a token"}'
r="$(rec_for why-does-the-archive-keep-secret-things-and-what)"
[ -n "$r" ] && ! grep -qF 'redacted' "$r" \
  || { echo "    prose about secrets was redacted; the archive must stay readable"; sed 's/^/    | /' "${r:-/dev/null}"; exit 1; }

# ---------------------------------------------------------------- private on disk
[ "$(file_mode "$P")" = 700 ] || { echo "    the archive directory is $(file_mode "$P"), not 700"; exit 1; }
for f in "$P"/*.json "$P"/*.md "$P"/*.yaml; do
  [ -e "$f" ] || continue
  [ "$(file_mode "$f")" = 600 ] || { echo "    $f is $(file_mode "$f"), not 600"; exit 1; }
done
# a file loosened by hand is a finding, and the repair is named
chmod 644 "$P"/20010101000000-legacy-alpha.json
expect_exit 10 "$MJ" doctor
expect_grep 'readable beyond their owner'
expect_exit 0 "$MJ" capture render
[ "$(file_mode "$P/20010101000000-legacy-alpha.json")" = 600 ] \
  || { echo "    capture render did not restore the record's mode"; exit 1; }

# ---------------------------------------------------------------- bounded
# The policy declares both bounds; neither has a default written beside a reader.
grep -qE '^prompts:' .ai/repo/policy.yaml || { echo "    the skeleton policy declares no prompts block"; exit 1; }
grep -qE '^  retention_max_days:' .ai/repo/policy.yaml
grep -qE '^  retention_max_bytes:' .ai/repo/policy.yaml
before="$(records)"
expect_exit 0 "$MJ" capture prune --dry-run
expect_grep 'would be pruned'
# the two planted records are dated 2001 and are over any sane age bound; the dry run wrote
# nothing, so they still carry their text
grep -qF 'a legacy prompt of alpha' "$P/20010101000000-legacy-alpha.md" \
  || { echo "    --dry-run pruned a body"; exit 1; }
expect_exit 0 "$MJ" capture prune
expect_grep 'body\(ies\) pruned'
[ "$(records)" = "$before" ] || { echo "    pruning deleted a record; it may only take the body"; exit 1; }
alpharec="$P/20010101000000-legacy-alpha.json"
[ "$(field "$alpharec" text)" = null ] || { echo "    the body was not pruned"; sed 's/^/    | /' "$alpharec"; exit 1; }
# everything that made the record worth keeping survived, including how it was linked
[ "$(field "$alpharec" episode)" = "\"$alpha\"" ] || { echo "    pruning lost the episode"; exit 1; }
[ "$(field "$alpharec" episode_link)" = '"session-context"' ] || { echo "    pruning lost the provenance"; exit 1; }
[ "$(field "$alpharec" pruned_bytes)" != "" ] || { echo "    pruning left no tombstone"; sed 's/^/    | /' "$alpharec"; exit 1; }
grep -q '"pruned_sha256"' "$alpharec" || { echo "    the tombstone carries no digest of what was taken"; exit 1; }
# the rendering says so rather than showing an empty block, which nobody can tell from a
# rendering that failed
grep -qF 'pruned by retention' "$P/20010101000000-legacy-alpha.md" \
  || { echo "    the rendering of a pruned record shows no tombstone"; sed -n '/## PROMPT/,$p' "$P/20010101000000-legacy-alpha.md" | sed 's/^/    | /'; exit 1; }
expect_no_grep 'a legacy prompt of alpha' "$P/20010101000000-legacy-alpha.md"
expect_no_grep 'a legacy prompt of alpha' "$P/20010101000000-legacy-alpha.yaml"
# a second prune has nothing left to do; a pruned record is not pruned again
expect_exit 0 "$MJ" capture prune
expect_grep '0 body\(ies\) pruned'
# the archive is complete and the pair invariant holds after all of it
expect_exit 0 "$MJ" capture render
expect_grep '0 rendering'

# ---------------------------------------------------------------- the doctrine
# A record that says nothing about its episode is the unrecoverable state, so it is the one
# doctor must refuse — reconcile is what closes it, and it is named as the repair.
printf '{\n  "schema": "majordomus.capture/v1",\n  "started_at": "20010102000000",\n  "provider": "claude-code",\n  "event": "UserPromptSubmit",\n  "id": "S1",\n  "session": "uuid-silent",\n  "source": null,\n  "cwd": null,\n  "repository": "r",\n  "branch": "b",\n  "head": "h",\n  "text": "a silent record"\n}\n' \
  > "$P/20010102000000-silent.json"
expect_exit 0 "$MJ" capture render >/dev/null
expect_exit 10 "$MJ" doctor
expect_grep 'say nothing about which episode'
expect_grep 'majordomus capture reconcile'
expect_exit 0 "$MJ" capture reconcile
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qF 'each name their episode or say why they cannot' "$T/doctor.out" \
  || { echo "    doctor does not report a fully accounted archive as accounted"; grep -i prompts "$T/doctor.out" | sed 's/^/    | /'; exit 1; }
# and the honest half: the count of unlinked records is reported, never rounded to zero
grep -qE 'unlinked with their provenance recorded' "$T/doctor.out" \
  || { echo "    doctor does not report how many records could not be linked"; exit 1; }

# ---------------------------------------------------------------- it never leaves the machine
# The archive has always been ignored; what is new is that nothing derived from it may reach
# the published half either. The gate that decides this is scripts/ci/prompt-privacy, and it
# is run here against this repository rather than trusted to exist.
expect_exit 0 bash "$ROOT/scripts/ci/prompt-privacy"
