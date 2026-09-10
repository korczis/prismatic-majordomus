# A server whose lease was taken over says so on the endpoint every reader probes, refuses to take on a new client, and reports that what its health describes is another process; the sessions it already had continue and it ends with them

## What it means

A lease changes hands while the process that held it is still running and still healthy. The usual cause is a rebuild: the election reads the lease's recorded executable, sees that the file it names has been replaced, and treats a server running code that is no longer on disk as stale. The superseded process is not killed — a client mid-answer should not lose its server because somebody ran `cargo build` — so it keeps its socket, its peer board, its execution history and the generation of the layer it loaded hours ago.

What it stops doing is being *the* server. Its index answers `leaseholder: false`; `lease::probe`, which every reader of a remembered address goes through, refuses it; a new `initialize` is refused with `409 lease_lost` naming the launcher as the way to the current server; and its own `health.report` warns that the server it describes is another process. The sessions it already had continue to be its own, and the process ends with them.

## How it works

`src/lease.rs` records the loss once, in `lost()`: a process-wide flag that is never unmade, the published lease document cleared, and the signal handler told to stop unlinking a file that is now somebody else's. `was_lost()` is the one question the rest of the executable asks, and it is deliberately not `held().is_none()` — a process that never took a lease at all (a test, a bridged client, a one-off command) must go on serving whoever asked it. `src/shared.rs` is what notices: the server's own reader compares the file's token with its own every `REAP_INTERVAL` and calls `lost()` the first time they differ.

The three answers follow from that one fact. `src/http/router.rs` writes `leaseholder` into the index under `lease::LEASEHOLDER_KEY`, and `lease::probe` reads it under the same constant, so the writer and the reader cannot drift apart. `src/http/mcp.rs` refuses an `initialize` that carries no session id when the lease is gone. `src/capability/builtin/health.rs` adds the finding to the `server` check and warns, because the standing it reports is accurate about a process that is not this one.

A server too old to answer the question at all is accepted by the probe. It cannot be told from a current one on that endpoint, and refusing it would be the worse failure: a live server taken for dead is taken over, which is how one checkout comes to have two.

## How to see it

```bash
just build
# start a server and note its address
apps/majordomus-cli/target/debug/majordomus mcp < /dev/null &
url="$(sed -n 's/.*listening on \(http:\/\/127\.0\.0\.1:[0-9]*\).*/\1/p' <<<"$(jobs)" )"
curl -s "$url/" | jq .leaseholder          # true
# take the lease away, as a rebuild's election would
rm .ai/local/state/mcp/server.json
sleep 1
curl -s "$url/" | jq .leaseholder          # false
curl -s -o /dev/null -w '%{http_code}\n' -X POST "$url/mcp" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"late","version":"0"}}}'
# 409
curl -s "$url/api/v1/health" | jq '.checks[] | select(.id=="server") | .findings'
```

## What it does not cover

The client that already holds the stale address is told there is no server there; it is not redirected to the one that is. Finding the current server is the launcher's election, which is where that knowledge belongs. A server predating this field is accepted by the probe, for the reason above, so the guarantee runs from this version forward rather than over every process on the machine. And nothing here shortens the superseded server's life: it ends when its last session does, which under an idle client is the session idle timeout.

## Why it exists

Measured in this repository on 2026-09-10. Two servers answered for one checkout: the leaseholder, and one whose executable had been replaced an hour earlier. Both reported the same name, the same `repository_id`, the same object and capability counts — and different registry fingerprints. A session whose environment carried the older address read a peer board with one peer on it while the board every other session shared had two, and nothing in any answer said which board it was reading. The takeover had worked exactly as designed; what nobody owned was the aftermath. The doctrine is `project.shared-server-resilience`, and `apps/majordomus-cli/tests/lease_lost.rs` proves the before and the after in one process, because a lease that was taken over is not given back.
