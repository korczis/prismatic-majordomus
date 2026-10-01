# majordomus-covers: none
# majordomus-timeout: 900
# claims: mesh-compaction-keeps-what-is-live
# Compacting the cooperation journal keeps what is live (docs/MESH.md, "Compaction"): a
# runtime compacted away while it slept is heard again when it wakes, whole, and the claim it
# took before it slept holds again on its peer and refuses a conflicting one there; who took
# a handover and who answered a review stay with them when the taker stops and the publisher
# does not; a handover somebody took keeps its stopped publisher nowhere, and one nobody took
# keeps it only for the handover retention.
#
# Why this runs the crate's two-runtime suite rather than two servers: no black-box knob
# reaches compaction in bounded time. A running server compacts every sixty heartbeats and
# keeps a dead stream for fifteen minutes (PEER_RETENTION), and a declaration sets only the
# heartbeat and the expiry, the knobs case 368 uses. So the runtimes are the library's own,
# linked through the real signed link protocol over an in-process transport
# (apps/majordomus-cli/tests/mesh_compaction.rs), and the compaction is the journal's own,
# called as the supervisor calls it with the retention at zero. Every test the claim rests on
# must be reported by name and pass: a test renamed or filtered away fails this case rather
# than letting it pass on nothing.
#
# Skips itself when cargo is absent, as the other cases that run the crate's suites do.
. "$ROOT/test/lib.sh"
command -v cargo >/dev/null 2>&1 || skip "cargo not installed"
MANIFEST="$ROOT/apps/majordomus-cli/Cargo.toml"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj493.XXXXXX")"; trap 'rm -rf "$S"' EXIT

RUSTFLAGS='' cargo test --manifest-path "$MANIFEST" --test mesh_compaction 2>"$S/cargo.log" >"$S/cargo.out" \
  || { tail -40 "$S/cargo.log" "$S/cargo.out"; echo "    the compaction suite failed"; exit 1; }
for t in \
  a_runtime_compacted_while_it_slept_is_heard_again_whole \
  who_took_a_handover_and_who_answered_a_review_outlive_the_takers_run \
  a_taken_handover_keeps_its_stopped_publisher_nowhere_and_an_untaken_one_only_so_long
do
  grep -qxF "test $t ... ok" "$S/cargo.out" \
    || { cat "$S/cargo.out"; echo "    $t did not run and pass"; exit 1; }
done
grep -q '^test result: ok\. 3 passed; 0 failed' "$S/cargo.out" \
  || { cat "$S/cargo.out"; echo "    the compaction suite ran something other than its three tests"; exit 1; }
echo "    ok: a runtime compacted while it slept is heard again whole; takers and answers outlive their runs; handovers pin only while needed"
