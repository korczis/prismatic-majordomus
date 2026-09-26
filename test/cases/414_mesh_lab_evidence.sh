# majordomus-covers: none
# The mesh lab's record is judged by something other than the lab: scripts/ci/mesh-lab-evidence
# reads target/mesh-lab/evidence.json and refuses an incomplete acceptance.
#
# `docs/CLAIMS.yaml` used to name test/mesh-lab/run as both the implementation and the test
# of `mesh-network-acceptance` — the script wrote its own verdict and nothing read it. This
# case covers the reader that now does: a complete record is accepted, and every way the
# record can be less than an accepted network is refused with exit 10 — a dropped scenario,
# a failed scenario, a pass that measured nothing, an overall `failed`, a foreign schema, an
# executable no node could name, a topology missing multicast, and a scenario the lab
# recorded that the reader does not know.
#
# It also holds the reader's required set equal to the scenarios test/mesh-lab/run reaches,
# so a scenario added to the lab and not to the reader (or the reverse) fails here rather
# than shrinking the proof in silence. The lab itself needs docker and four containers and
# runs in CI's mesh-lab job; what is here needs neither, and is what makes that job's
# success mean something.
. "$ROOT/test/lib.sh"

JUDGE="$ROOT/scripts/ci/mesh-lab-evidence"
expect_file "$JUDGE"

# The scenarios the lab actually reaches, read out of the script itself.
grep -oE '[[:space:]]pass [a-z_]+ "' "$ROOT/test/mesh-lab/run" | awk '{print $2}' | LC_ALL=C sort -u > "$T/lab-scenarios"
"$JUDGE" --scenarios | LC_ALL=C sort -u > "$T/required-scenarios"
[ -s "$T/lab-scenarios" ] || { echo "    no scenario names were read out of test/mesh-lab/run"; exit 1; }
diff -u "$T/required-scenarios" "$T/lab-scenarios" || {
  echo "    scripts/ci/mesh-lab-evidence requires a different set than test/mesh-lab/run runs"
  exit 1
}

# A complete record: what a green lab writes.
scenarios=""
for name in $(cat "$T/required-scenarios"); do
  scenarios="$scenarios$(printf '"%s":{"verdict":"pass","detail":"measured, 42ms"},' "$name")"
done
write_evidence() { # file  jq-filter-applied-to-the-complete-record
  jq "$2" > "$1" <<JSON
{"schema":"mesh-lab-evidence/v1",
 "started_at":"2026-09-20T06:00:00Z","finished_at":"2026-09-20T06:04:00Z",
 "status":"passed","executable":"majordomus 0.8.0","platform":"Linux aarch64",
 "protocols":{"discovery":2,"link":{"min":1,"max":1},"event":1},
 "topology":{"nodes":["a","b","c","x"],"network":"one docker bridge per run",
             "transports":["udp_multicast","http"],
             "repository":{"a":"lab","b":"lab","c":"lab","x":"other"}},
 "scenarios":{${scenarios%,}}}
JSON
}

write_evidence "$T/ok.json" .
expect_exit 0 "$JUDGE" "$T/ok.json"
expect_grep 'OK .*scenarios passed'

# No record at all is not a pass: a lab that never ran cannot accept a network.
expect_exit 2 "$JUDGE" "$T/absent.json"
expect_grep 'no record at'

# Each way of being incomplete, refused with exit 10 and named.
write_evidence "$T/dropped.json" 'del(.scenarios.network_partition)'
expect_exit 10 "$JUDGE" "$T/dropped.json"
expect_grep 'scenario network_partition is absent'

write_evidence "$T/failed-scenario.json" '.scenarios.cross_node_claim.verdict = "fail"'
expect_exit 10 "$JUDGE" "$T/failed-scenario.json"
expect_grep 'scenario cross_node_claim: fail'

write_evidence "$T/silent-pass.json" '.scenarios.cli_verify.detail = ""'
expect_exit 10 "$JUDGE" "$T/silent-pass.json"
expect_grep 'cli_verify passed without saying what it measured'

write_evidence "$T/failed.json" '.status = "failed"'
expect_exit 10 "$JUDGE" "$T/failed.json"
expect_grep "verdict is 'failed'"

write_evidence "$T/schema.json" '.schema = "mesh-lab-evidence/v2"'
expect_exit 10 "$JUDGE" "$T/schema.json"
expect_grep 'not mesh-lab-evidence/v1'

write_evidence "$T/unknown-bin.json" '.executable = "unknown"'
expect_exit 10 "$JUDGE" "$T/unknown-bin.json"
expect_grep "executable is 'unknown'"

write_evidence "$T/undated.json" 'del(.finished_at)'
expect_exit 10 "$JUDGE" "$T/undated.json"
expect_grep 'finished_at is missing'

write_evidence "$T/no-multicast.json" '.topology.transports = ["http"]'
expect_exit 10 "$JUDGE" "$T/no-multicast.json"
expect_grep "transport 'udp_multicast'"

write_evidence "$T/three-nodes.json" '.topology.nodes = ["a","b","c"]'
expect_exit 10 "$JUDGE" "$T/three-nodes.json"
expect_grep 'topology names 3 node'

write_evidence "$T/extra.json" '.scenarios.a_scenario_nobody_requires = {"verdict":"pass","detail":"x"}'
expect_exit 10 "$JUDGE" "$T/extra.json"
expect_grep 'a scenario this reader does not require'

echo "    ok: the lab's record is judged from outside the lab, and every incomplete record is refused"
