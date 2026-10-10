+++
title = "Runtime baseline: server, MCP, mesh, surfaces, coordination, failure, release"
description = "forensic finding, 2026-10-09 at 1fb8d6be44: the shared server, MCP transports, mesh discovery, trust and journal, projections, Cockpit, coordination, failure handling, gates, update and release, each read against its documents; the capability matrix with owner, fault path and test, the drift between documents and code, the premises of the runtime pack that the code refutes, and the issue that addresses each gap"
weight = 40
[extra]
source = "docs/RUNTIME_BASELINE.md"
+++

{% raw %}

At master 1fb8d6be44, 2026-10-09.

This is the M01 deliverable of the owner's runtime prompt pack
(`tmp/owner-packs-2026-10-09/majordomus-runtime-prompt-pack/`). It merges four read-only audit
slices taken at that commit: lifecycle and MCP transports, mesh, projections/Cockpit/coordination,
and failure/enforcement/update/release. No build, test or server control was run for them. Live
facts come from GET requests to the shared server at 127.0.0.1:61068, from read-only `gh` calls,
and from reading `.ai/local/state/mcp/`. "Unverified" means reading could not confirm it.

Every gap is linked to the plan record that addresses it. Records written for this baseline are
I2126-I2165 and the milestones runtime-lifecycle, mesh-journal-converges, work-is-coordinated,
mcp-transports-are-reliable, surfaces-never-disagree, cockpit-control-center, failure-is-visible,
architecture-is-enforced and safe-self-update (orders 136-144). Existing records referenced:
I2123 and I2124 (milestone runtime-trust, order 135, on branch fix/a-remote-caller-cannot-write),
I2059, I1502, I1504, I1955, I1990, I0906 on master, and I2071, I2072, I2096-I2103 and I2115 on
unmerged branches.

## Legend

- `src/` = `apps/majordomus-cli/src/`, `tests/` = `apps/majordomus-cli/tests/`. Every other path
  is from the repository root.
- Status: I implemented, P partial, A absent, M misdocumented, D duplicated.
- "sub-audit" marks a claim a subordinate audit made and the slice author did not re-check.
- "Refuted" marks a premise of the pack that the code contradicts.

## 1. Capability matrix

### 1.1 Shared server lifecycle

<div class="overflow-x-auto" tabindex="0">

| capability | canonical owner | healthy path | fault path | test | status | gap | issue id |
|---|---|---|---|---|---|---|---|
| Lease location and format | `src/lease.rs:34,37,276-312` (`LeaseDocument`, schema `majordomus-mcp-lease/v1`); one reader `LeaseFile::read` `src/lease.rs:349-380` | typed read with 4 variants | absent, empty, corrupt and foreign schema classified | `tests/mcp_shared.rs::a_corrupt_lease_is_taken_over`, `::a_lease_of_another_schema_and_an_old_empty_file_are_taken_over`; `tests/health_server.rs::the_check_and_the_status_read_the_lease_through_one_function`; `test/cases/110_lease_reader.sh` | I | none material | — |
| Election, cold-start race | `src/lease.rs:506-591`, `O_CREAT\|O_EXCL` at `src/lease.rs:516-520`; callers `src/commands/mcp.rs:137,238`, `src/commands/serve.rs:72` | first `create_new` wins | concurrent creators: one succeeds | `tests/mcp_shared.rs::two_clients_starting_together_share_one_server`, `::a_storm_of_clients_converges_on_one_server_and_one_board`; `tests/serve_lifecycle.rs::three_ensures_at_once_share_one_server`; cases 191, 192 | I | none for the create step | — |
| Lease publish (URL) | `Lease::publish` `src/lease.rs:1082-1107` (tmp + rename, no fsync); initial write `src/lease.rs:543` (plain `write_all`, no fsync) | URL written only after workers serve (`start_published` `src/shared.rs:294-305`) | taken-over lease refuses to publish (`is_mine` `src/lease.rs:1088`) | `src/shared.rs::tests::publication_observes_a_serving_socket_and_failure_closes_it`; `tests/shared_units.rs::the_lease_is_held_probed_published_and_released` | P | `is_mine` → write → rename not atomic; fixed tmp name `server.json.tmp`; no fsync | I2128 |
| URL-before-ready race | `src/shared.rs:176-184` (mesh and surfaces resolved before publish) | published URL answers | publication failure closes the socket | same as above | I | — | — |
| Stale, corrupt, abandoned lease | `inspect` `src/lease.rs:714-765`; `take_over` `src/lease.rs:778-798` | stale → take over | compare-then-unlink | `tests/mcp_shared.rs::a_stale_lease_is_taken_over`, `::an_abandoned_lease_is_taken_over_without_waiting_when_it_is_old`; `tests/serve_lifecycle.rs::a_stale_lease_and_a_killed_server_are_both_recovered`; case 113 | P | read, compare, `remove_file` not atomic: two electors can unlink each other's fresh lease; loser degrades to `--standalone` (`src/commands/mcp.rs:139-141`); no test of the interleaving (unverified that it occurs) | I2128 |
| Busy vs dead owner | `alive` `src/lease.rs:928-938` (`kill(pid,0)`); `Found::Busy` `src/lease.rs:742`, grace `src/lease.rs:555-568` | live silent owner waited `busy_grace` | dead pid → immediate takeover | `tests/lease_busy.rs` (7 tests); `tests/serve_lifecycle.rs::a_live_owner_that_answers_late_keeps_its_lease`, `::a_live_owner_that_never_answers_is_still_taken_over` | P | probe is `GET /` on the same 4-worker pool (`src/http/server.rs:23`) that runs `tools/call`; four calls longer than ~12 s make a healthy server look wedged; `is_own` (`src/lease.rs:429`) fixes only the self-probe | I2126 |
| Identity / generation fencing | token per election `src/lease.rs:522-529`; tick compares token `src/shared.rs:205-215`; probe checks name, repository_id, leaseholder `src/lease.rs:902-905`; `GET /` sets leaseholder `src/http/router.rs:710` | a lost server answers `leaseholder:false` and 409 to new initialize (`src/http/mcp.rs:119-128`) | yes | `tests/lease_lost.rs::a_server_that_lost_its_lease_says_so_and_takes_on_nobody_new`; `tests/mcp_shared.rs::a_bridged_peer_re_attaches_when_another_process_took_the_lease_first` | P | probe compares no token, pid or started_at; `serve stop` SIGTERMs `doc.pid` after a probe proving only "some leaseholder answers" (`src/commands/serve.rs:802-814`); a lost server says `leaseholder:true` for up to 500 ms | I2126, I2128 |
| Superseded lease (binary replaced) | `superseded` `src/lease.rs:695-712`; `ExecutableIdentity::replaced` `src/lease.rs:232-256`; `ServerStanding::Outdated` `src/capability/builtin/server.rs:403-422`; `serve ensure` same-path replace `src/commands/serve.rs:554-570` | new election takes over, old marks itself lost | yes | `tests/shared_units.rs::a_server_whose_binary_was_replaced_loses_the_lease`; `tests/serve_lifecycle.rs::a_server_of_this_executable_replaced_on_disk_is_replaced`; case 885 | P | lost server keeps existing sessions (`src/http/mcp.rs:113-118`); bridges ping every 20 s (`src/mcp/bridge.rs:19`) and count as attached (`src/commands/serve.rs:136-146`), so it never ends; only same-path replacement is detected | I2127, I2163, I2059 |
| Signals / SIGKILL | handler `src/lease.rs:1164-1212` (unlink, re-raise `SIG_DFL`) | lease removed, process dies of the signal | SIGKILL: lease left, URL dead → Stale → takeover | `tests/mcp_shared.rs::sigterm_removes_the_lease_before_the_process_dies`, `::sigint_and_sighup_remove_the_lease_before_the_process_dies`; case 113 | P | SIGTERM not graceful: `SharedServer::stop` (`src/shared.rs:282-289`) never runs, episodes left open, answers cut; unlink is unconditional on HELD, so it can remove a successor's lease within 500 ms of a takeover | I2128 (I0906 for the container) |
| Idle shutdown | `serve --idle` `src/commands/serve.rs:128-146`; `DEFAULT_IDLE_SECONDS=900` `src/cli.rs:3427`; `wait_until_peers_leave` `src/shared.rs:256-273` | stops after N s with no MCP session | — | `tests/serve_lifecycle.rs::an_idle_server_ends_by_itself`; `tests/http_serve.rs::with_stdin_at_dev_null_the_server_keeps_running` | P | "attached" = MCP HTTP sessions only (`src/shared.rs:250-253`): a Cockpit-only user is not counted; idle value is a compiled constant, absent from `policy.yaml server:` | I2131, I2129 |
| Port choice / collision | `DEFAULT_PORT=8741` `src/cli.rs:3301`; `bind_or_fallback` `src/http/server.rs:77-86`; non-loopback warning `src/http/server.rs:39-49` | 8741 or a random free port | taken → random, logged | `tests/mcp_shared.rs::a_taken_port_falls_back_to_a_free_one_and_serve_defers_to_the_running_server`; `tests/serve_lifecycle.rs::ensure_takes_a_free_port_when_the_one_it_asks_for_is_taken`, `::a_non_loopback_bind_is_warned_about` | P | only one checkout gets 8741; others get a URL that changes on restart (live: 61068) | I2131 |
| Lease timings declared in policy | `Timings` `src/lease.rs:89-191`; `declare_timings` only from `LoadedPolicy::load` `src/policy.rs:364`; `.ai/repo/policy.yaml:335-339` | — | — | `tests/lease_declared.rs` (calls `declare_timings` directly); case 354 (schema only) | M | declaration never reaches `mcp` or `serve` (OnceLock filled at `src/lease.rs:1043` before `App::load`, `src/commands/mcp.rs:173-174`, `src/commands/serve.rs:72,89-90`); `standing_of` (`src/capability/builtin/server.rs:370,386`) and spawn claim (`src/commands/serve.rs:330,358`) use compiled `BIND_GRACE`; latent (values equal defaults) | I2129 |
| Lifecycle commands | `serve` / `serve status\|ensure\|stop` (`src/cli.rs` `ServeCommand` ~3440-3494); `mcp` `src/commands/mcp.rs:24-46`; capability `server.status` only `src/capability/builtin/server.rs:613-633` | ensure → converge → ready (0) or 10 | ensure reports absent, starting, ready, outdated, stale | `tests/serve_lifecycle.rs::ensure_starts_one_server_finds_it_the_next_time_and_stop_ends_it`, `::stop_answers_for_the_server_it_named_even_when_the_lease_is_taken_again_at_once`, `::stop_leaves_a_server_of_another_checkout_alone`; cases 108, 190 | P | ensure and stop are CLI-only by stated design; no restart, no server doctor (`lib/doctor.sh` does not look at the server), no `ui` command | I2157, I2131 |
| Lifecycle state model | `ServerStanding {absent, starting, ready, outdated, stale}` `src/capability/builtin/server.rs:163-177`; `standing_of` `src/capability/builtin/server.rs:362-425` | — | — | `tests/server_status.rs::the_standing_is_decided_from_the_lease_the_probe_and_the_version`, `::a_server_asked_about_itself_by_every_worker_at_once_still_says_ready` | P | an observer's classification, not a server-owned state; no degraded, stopping, failed; degraded only as client mode `Backend::Alone` (`src/commands/mcp.rs:154-164`) | I2130 |
| UI-first startup | `serve ensure`, `env enter` (`src/commands/serve.rs:481-576`, `src/commands/env.rs:701-760`); Cockpit mounted `src/shared.rs:166-168` | works without stdio | — | case 190; `tests/serve_lifecycle.rs::ensure_starts_one_server_...` | P | works; no `majordomus ui` (refuted premise), Cockpit-only use does not keep the server alive, URL unstable | I2131 |
| Server observability (log) | `spawn_server` `src/commands/serve.rs:699-764` | stderr → `server.log` (append) | — | none | P | inherited `MAJORDOMUS_LOG` silences the server (live: `error`, nothing logged for 8 h); never rotated; `self_link` WARN every ~31 s; MCP-owned server logs to the client's stderr only | I2130 |
| Launcher build in the primary | `bin/majordomus-cli:35-59` builds when stale; `bin/majordomus-mcp:20` execs it | — | `MAJORDOMUS_NO_BUILD`, `MAJORDOMUS_BIN` opt-outs | case 90 | P | an MCP client in the primary with stale sources runs `cargo build`, replaces the leased binary, supersedes the lease; the hook path refuses (`lib/capture.sh:1599-1612`), the MCP path does not | I2059 (existing) |

</div>


### 1.2 MCP transports and projection

<div class="overflow-x-auto" tabindex="0">

| capability | canonical owner | healthy path | fault path | test | status | gap | issue id |
|---|---|---|---|---|---|---|---|
| stdio transport, stdout hygiene | `src/mcp/stdio.rs:32-70`; launcher `bin/majordomus-mcp:20`; build output to stderr `bin/majordomus-cli:54-59` | one JSON object per line | bad UTF-8/JSON → -32700, EPIPE ends | `tests/mcp_stdio.rs::stdout_is_protocol_only_even_at_trace_level_with_a_degraded_layer`, `::a_client_that_closes_its_read_end_ends_the_session`; `tests/shared_units.rs::the_stdio_loop_survives_bad_bytes_and_stops_on_a_broken_pipe`; `tests/mcp_conformance.rs::malformed_traffic_is_answered_and_the_session_survives` | I | no line-length bound on stdin (`src/mcp/stdio.rs:40`) | I2147 |
| stdio → shared bridge | `Bridge` `src/mcp/bridge.rs:170-367`; backends `src/commands/mcp.rs:63-372` | one POST per message, 20 s heartbeat | 404 → re-initialize and replay announcements (`src/mcp/bridge.rs:216-291`); error → `failover` re-elects (`src/commands/mcp.rs:229-311`) | `tests/mcp_shared.rs::a_bridge_is_transparent_and_a_restarted_server_answers_the_same_bytes`, `::a_bridged_peer_takes_over_when_its_server_dies`, `::an_announcement_outlives_the_server_it_was_made_to`; `tests/shared_units.rs::the_bridge_client_speaks_to_a_real_socket_and_names_every_failure` | P | non-idempotent replay: 60 s timeout (`src/mcp/bridge.rs:25`) → failover → same URL → resends the same `tools/call` (`src/commands/mcp.rs:301`, `:283`); any non-404 4xx re-elects (`src/mcp/bridge.rs:352`); synchronous under a mutex (`src/commands/mcp.rs:218`) | I2145 |
| Streamable HTTP endpoint | `src/http/mcp.rs` (POST/DELETE, `Mcp-Session-Id`, GET → 405) | initialize → session → POST → DELETE 204 | unknown session 404; missing 400; init after loss 409 `lease_lost` | `tests/mcp_shared.rs::mcp_over_http_directly_with_sessions`, `::mcp_over_http_refuses_malformed_traffic_and_keeps_serving`; `tests/mcp_conformance_http.rs::interleaved_http_sessions_keep_their_own_identity_and_one_listing`; `tests/shared_units.rs::the_endpoint_opens_reaps_and_closes_sessions` | P | request half only (no SSE, documented); `MCP-Protocol-Version` and `Accept` ignored; predictable ids `pid-seq-subsecnanos` (`src/http/mcp.rs:159`); per-session `Mutex<Server>` (`:139`); 90 s reaper can expire a running request (`:138`) | I2147 |
| Cancellation | `notification()` ignores all but `initialized` (`src/mcp/protocol.rs:234-238`) | — | — | none | A | `notifications/cancelled` dropped; bridge cannot forward it while a call is in flight; `executions.cancel` not wired | I2146 |
| Payload limits | request body `MAX_BODY_BYTES=1 MiB` → 413 (`src/http/server.rs:20,207-214`) | — | 413 | `tests/http_serve.rs::errors_are_typed_and_the_server_survives_them` (unverified that it covers 413) | P | owner's stdio session unbounded; >1 MiB bridged message → 413 → re-election → resend → misleading -32603; response bodies unbounded (`src/mcp/bridge.rs:85`) | I2147, I2145 |
| Tools and resources from one registry | `Surface` `src/mcp/surface.rs:1-4,367-394`; `METHODS` `src/mcp/protocol.rs:82-90`; annotations from effect `src/mcp/protocol.rs:446-456` | `tools/list` = registry walk | refusal code in `_meta` | `src/mcp/protocol.rs::tests::a_tools_annotations_are_its_capabilitys_hints_and_a_writer_is_never_announced_as_safe`; `tests/mcp_conformance.rs::every_listed_tool_is_one_a_client_accepts`; `tests/http_serve.rs::mcp_and_http_answer_the_same_capability_with_the_same_result`; case 940 | I | none; the pack's premise of MCP-specific lists is **refuted** | — |
| Authorization of writers | none at MCP/executor: `Surface::call` `src/mcp/surface.rs:380-393` → `ctx.execute`; executor counts writes `src/capability/executor.rs:160-166`; HTTP browser-origin check `src/http/router.rs:504-508,718-745` | — | foreign `Origin` → 403 | `tests/mcp_conformance_http.rs::a_foreign_origin_can_neither_open_nor_use_an_mcp_session` | A | no authentication on any surface; `executions.start` runs any capability incl. the six writers with no effect gate (`src/execution/engine.rs:206-223`); LAN/tailnet-reachable when bound beyond loopback (`.ai/repo/mesh/majordomus.yaml:20-48`) | I2123 (remote); local laundering not placed, see INDEX |
| Provider launch configs | hand-written `.mcp.json`, `.gemini/settings.json`, `.codex/config.toml` (commit d1f1efbfbe, 2026-09-05) | each execs `bin/majordomus-mcp` | — | `src/capability/builtin/mcp.rs` tests at `:384-392`, `:720-735`; `tests/providers_projection.rs` (unverified scope) | P | not generated (adopter `init` gets none); diagnosis is a substring (`src/capability/builtin/mcp.rs:394-399`) fooled by a comment in `.codex/config.toml:2-3`; client timeouts 120 s vs bridge 60 s | I2148 |
| MCP protocol versioning | `src/mcp/protocol.rs:71` `PROTOCOL_VERSIONS` | negotiate | fall back to newest | `tests/mcp_conformance.rs:295` | I | — | — |

</div>


### 1.3 Mesh

<div class="overflow-x-auto" tabindex="0">

| capability | canonical owner | healthy path | fault path | test | status | gap | issue id |
|---|---|---|---|---|---|---|---|
| Node identity | `src/mesh/identity.rs:159-238` | load or create `node.json`; node id = digest(pk) | malformed file is an error, not replaced | `identity.rs` tests (7) incl. `a_malformed_identity_file_is_an_error_not_a_replacement` | I | no rotation or revocation; 0600 set after `fs::write` (`src/mesh/identity.rs:338-347`); non-strict `verify` (`:262`, intent unverified) | I2134 |
| Discovery envelope | `src/mesh/protocol.rs:225-266` | size → JSON → version → bounds → skew → signature | typed `Refusal`, counted (`src/mesh/manager.rs:71-81`) | `protocol.rs` tests + proptest `parse_never_panics_on_any_input` (`:399`) | I | v1 still accepted (documented, `src/mesh/protocol.rs:237-246`); no v1 fixture test | I2164 |
| Provider contract | `src/mesh/provider.rs:197-206` | raw bytes over `Sender<Observation>` | provider failure is a status (`src/mesh/manager.rs:451-455`) | `manager.rs::a_synthetic_provider_reaches_the_registry_with_zero_registration` | I | channel unbounded (`src/mesh/manager.rs:425`); no rate limit, no drop counter; one Ed25519 verify per datagram on one thread | I2133 |
| Multicast | `src/mesh/multicast.rs:173-258` | join on INADDR_ANY, declared TTL, beacon + 2 s jitter | bind/join failure → Failed | `multicast.rs` tests (delivery best effort, `:341-350`) | P | IPv4 only; default interface only (`:95`); TTL and group barely validated (`src/mesh/config.rs:167-168`) | not placed |
| Broadcast | `src/mesh/broadcast.rs:92-202` | off by default; listens when multicast is off | bad address → Failed | `broadcast.rs::destinations_follow_the_declared_mode` | I | IPv4 only | not placed |
| Rendezvous client | `src/mesh/rendezvous.rs:87-180` | POST signed envelope, candidates → channel | backoff ×2 up to ×8 + jitter (`:127-134`) | `rendezvous.rs::a_reachable_endpoint_yields_registrations_and_candidate_observations`; `tests/mesh.rs::two_runtimes_discover_each_other_through_the_rendezvous_handshake` | P | reply read unbounded (`src/mcp/bridge.rs:84-85`); candidate count unbounded; endpoints unvalidated | I2133, I2147 |
| Rendezvous server (`mesh.register`) | `src/mesh/manager.rs:589-637` | verify, record, answer own + ≤32 candidates | refusal string | `manager.rs::registration_records_and_answers_with_own_envelope`, `registration_refuses_garbage_and_an_inactive_mesh_says_so` | P | answers candidates even when the caller is refused (`src/mesh/manager.rs:623-636`); candidates of any repo and trust (`src/mesh/registry.rs:349-353`) | I2133 |
| Registry | `src/mesh/registry.rs:237-406` | one record per (node, runtime); replay by (instance, seq) | 256 cap, evict longest-unseen non-allowlisted (`:291-305`); TTL 60 s, retention 15 min | `registry.rs` tests (5); `tests/mesh.rs::a_server_activates_the_mesh_and_registration_converges_to_one_record` | I | older instance within skew accepted as "restart" (`:252-260`), cosmetic | not placed (minor) |
| Seeds and local worktrees | `src/mesh/cooperation.rs:2493-2511` | seeds dialed; worktrees share the node key (`:1667-1670`) | dial refusal recorded | `tests/mesh_cooperation.rs::two_worktrees_of_one_machine_are_two_runtimes_under_one_key` | I | seeds bypass the registry for dialing by design; handshake still gates | — |
| Trust policy | `src/mesh/trust.rs:92-116` | allowlist string-equal; deny_unknown → Observed | — | `trust.rs` tests (5) | P | key-change rejection dead (`:99-103`), TOFU trusts every key; exact-string allowlist; frozen at activation (`src/mesh/manager.rs:466,486`, `src/mesh/cooperation.rs:1150`) | I2136, I2134 |
| Trust root | `src/shared.rs:34-48,164-174` | declaration read once at start | — | none | P | read from the working tree, so a branch can add keys, seeds, endpoints or tofu (T12) | I2135 |
| Handshake (hello/welcome) | `src/mesh/cooperation.rs:1765-1953` (answer), `:2172-2325` (dial) | checks of `docs/MESH.md:128-136` | typed `RefusalCode`, counted, ≤128 listed | `cooperation.rs::isolation_version_trust_and_replay_are_typed_refusals`; `tests/mesh_cooperation.rs::malformed_forged_and_future_link_messages_are_typed_refusals_over_http`, `runtimes_of_different_repositories_never_link_and_say_why`, `a_reachable_but_untrusted_key_is_refused` | I | hello names no audience (`src/mesh/link.rs:444-456`), replays to another runtime within ±300 s; nonce cache in memory, checked after trust (`src/mesh/cooperation.rs:1847-1875`) | I2132 |
| Sync round | `src/mesh/cooperation.rs:2006-2102` (answer), `:2380-2489` (dial) | link id + rising counter + signature; marks + missing events | `unknown_link` → re-hello; replay; signature | `cooperation.rs::a_sync_under_an_unknown_link_or_a_replayed_counter_is_refused` | P | 600 KiB budget ignores marks (`src/mesh/journal.rs:1734-1766`), can exceed 900 KiB `MAX_LINK_MESSAGE` (`src/mesh/link.rs:67,70`) (arithmetic, unverified at runtime); dialer's reply unbounded | I2139, I2133 |
| Link supervision | `src/mesh/cooperation.rs:2513-2658` | one worker per target; connected, degraded, unreachable, expired | backoff `heartbeat·2^f` capped (`:2611-2614`) | `tests/mesh_cooperation.rs::a_killed_runtime_expires_and_its_restart_reconnects_as_the_same_runtime` | I | no jitter; `UnknownLink` retries with zero delay (`:2592-2595`), a busy loop | I2133 |
| Journal append/ingest | `src/mesh/journal.rs:1464-1504`, `:1807-1959` | dense seq, Lamport, signature, repo, bounds, origin trust, gap buffer | typed `Rejection`; quotas per node and stream | `journal.rs` tests (26); proptests `any_permutation_with_duplicates_converges`, `hostile_json_never_panics` | P | lamport unbounded and unvalidated (`:1952-1953`); no equivocation detection when two events share `(stream, seq)` (`:1929`) | I2124 (lamport); equivocation not placed |
| Beats / liveness | `src/mesh/journal.rs:1529-1539`, `:1644-1699`, `:2012-2066` | signed beat counter; age aged on local monotonic clock | relayed age clamped to expiry+1 s (`:1646,1693-1696`) | `journal.rs::beats_relay_freshness_without_comparing_clocks`, `a_relay_cannot_mint_a_beat_and_an_untrusted_origin_is_not_heard` | I | `age_ms` unsigned: a trusted relay can present an old beat as fresh to a newcomer for up to one expiry | not placed |
| Persistence | `src/mesh/journal.rs:1407-1423` (reload), `:1961-1985` (append), `:2250-2264` (rewrite); path `src/shared.rs:119` | append each accepted event; reload re-ingests | write errors ignored (`let _`, `:1984`); no fsync | `journal.rs::a_reloaded_journal_keeps_events_and_resurrects_no_liveness` | P | reload bypasses origin trust (`:1419`); unparsable lines dropped silently (`:1408-1413`); torn tail fuses the next event (`:1969-1984`); unbounded `read_to_string`; umask permissions | I2137, I2124 |
| Compaction | `src/mesh/journal.rs:2199-2373`; every 60 ticks (`src/mesh/cooperation.rs:1498-1501`) | drop dead streams; tombstones in memory | handover pin ≤24 h; taker/answerer closure | proptest `compacting_a_crowded_node_changes_nothing_live`; `tests/mesh_compaction.rs` (3); case 493 | I | tombstones not persisted (documented); overflow evicts lexicographically first (`:2238-2242`) | not placed (minor) |
| Fold / state digest | `src/mesh/state.rs:502-750` | order `(lamport, stream, seq)`, liveness, exclusivity | BTreeMap dedup | proptests `fold_is_order_and_duplicate_independent` (one fixed 8-event history), `exclusivity_holds_for_any_interleaving` | I | digest diverges during compaction (documented, `docs/MESH.md:296-305`); a conflicted claim silently becomes Held when the winner ends; no randomized multi-stream history | I2138 |
| Claim admission | `src/mesh/cooperation.rs:3300-3334`, `src/mesh/state.rs:815-834` | local fold check under `admission` mutex | `claim_conflict` (422 / exit 10) | `state.rs::a_remote_claim_is_seen_and_refuses_a_conflicting_admission`; `tests/mesh_cooperation.rs::two_runtimes_link_and_share_sessions_claims_reviews_and_a_handover`; case 368 | P | no fencing; expired claim revives and wins (`src/mesh/journal.rs:1681-1697`, `src/mesh/state.rs:656-661,677-703`); no notification Held→Conflicted | I2138 |
| Release | `src/mesh/cooperation.rs:3373-3390` | stream-prefix check | `not_own` | `cooperation.rs::a_release_is_the_holders_and_a_session_close_ends_its_claims` | P | ownership per run, not per session | I2138 |
| Handover publish/consume | `src/capability/builtin/mesh.rs:863-965`, `src/mesh/handover.rs:314-384` | read only under `.ai/local/state/handovers/`; materialise once | path canonicalised and confined | `handover.rs::a_hostile_handover_is_written_inside_the_directory_with_no_injected_keys`; case 368 | I | unauthenticated over HTTP beyond loopback | I2123 |
| Board projection | `src/mesh/cooperation.rs:1332-1430` | attached peers → `board-<peer>` sessions + advisory claims | detach closes | `cooperation.rs::the_board_is_projected_into_sessions_and_advisory_claims_and_withdrawn_with_it` | I | sets no task or issue (`:1345-1357`) | I2144 |
| Mesh doctor | `src/mesh/doctor.rs:140-340` (`runtime_check` `:372`) | declaration, identity, trust, repo, udp, multicast, protocol, link, runtime | typed fail with remedy | `doctor.rs` tests (9); `tests/mesh.rs::an_enabled_declaration_the_server_could_not_activate_fails_the_doctor_and_names_why`; case 494 | I | does not check a beyond-loopback HTTP bind while the mesh is enabled | not placed (reduced by I2123) |
| Repository identity | `src/mesh/repository.rs` `resolve` | digest of `rev-list --max-parents=0 HEAD` or declared | shallow or empty repo refused | `repository.rs` tests (3); `tests/mesh_cooperation.rs::runtimes_of_different_repositories_never_link_and_say_why` | P | depends on HEAD (this repo: 4 roots across `--all`, 1 on HEAD); resolved once; public fingerprint on the LAN | not placed |
| `mesh.events` paging | `src/capability/builtin/mesh.rs:601-619`, `src/mesh/journal.rs:2124-2133` | filter `lamport > after`, take `limit` | — | none | M | `EventList.lamport` documented as next cursor (`src/capability/builtin/mesh.rs:594-596`) but is the max lamport; pages skip events | I2140 |
| Mesh surfaces | `src/capability/builtin/mesh.rs:1092-1365` | one module | — | case 301 (`scripts/ci/mesh-check`) | I | — | — |
| Mesh lab | `test/mesh-lab/run` (4 Linux containers); CI job `mesh-lab` (`.github/workflows/validate.yml:873-885`) | multicast, links, isolation, claims, partition/heal, crash/restart | — | the lab | P | runs only when `needs.plan.outputs.mesh_lab == 'true'`; scheduled full plan has not been green for 10+ runs | I2159 |

</div>


### 1.4 Projections, Cockpit and coordination

<div class="overflow-x-auto" tabindex="0">

| capability | canonical owner | healthy path | fault path | test | status | gap | issue id |
|---|---|---|---|---|---|---|---|
| Capability declaration | `capability!` `src/capability/handler.rs:485`; `Capability` `src/capability/model.rs:1006`; `src/capability/module.rs:86,110` | one declaration → id, schema, exposure, effect, availability (`src/capability/handler.rs:500-528`) | duplicate id / namespace / cache refused (`src/capability/registry.rs:73`) | `tests/properties.rs:419 any_composition_that_repeats_an_id_is_refused_naming_both_parties`; `tests/registry.rs` | I | — | — |
| Single executor | `Context::execute` `src/capability/handler.rs:291` → `src/capability/executor.rs:92` → `registry.dispatch` `src/capability/registry.rs:812` | only dispatch caller outside the registry is `src/capability/executor.rs:156` | typed errors `src/capability/handler.rs:76` → 400/404/422/500 `src/http/router.rs:903-913` | `tests/properties.rs:171`; `tests/executor.rs` | I | 89 `LOCAL` CLI commands bypass it (`src/cli/local.rs:137`) | not placed (SURFACES F-4) |
| HTTP API routes | `src/http/router.rs:814 capability()`, index `:778 routes()` | live: 181 routes = 181 OpenAPI operations | 404/405 (`:820-845`); foreign Origin for non-GET (`:503-508,718`) | `tests/projections.rs:26`; `tests/http_serve.rs:17,131`; `tests/cockpit.rs:747` | I | — | — |
| OpenAPI | `src/http/openapi.rs`; `docs/generated/openapi.{json,yaml}` (`src/generate.rs:59-61`) | live = committed, 181 = 181 | component collisions renamed (`src/capability/schema.rs:272-288`) | `tests/projections.rs:26`; `tests/generate_check.rs`; case 92 | I | Swagger page: no CSP, no SRI (`src/http/swagger.rs:52,79`; sub-audit) | I2151 |
| MCP tools/resources | `src/mcp/surface.rs:367 tools()`, `:294 resources()`; `src/mcp/protocol.rs:339 instructions()` | present ⇔ declared | annotations follow effect | `tests/projections.rs:26`; `tests/mcp_conformance_projection.rs:215,288`; cases 940, 620 | I | — | — |
| CLI (Rust) | second declaration: clap `src/cli.rs:37`; `CliExposure` `src/capability/model.rs:874` | 101 of 184 executables carry a CLI path (live) | claim with no clap command is a finding (`src/capability/closure.rs:205,320`); unbacked commands in `LOCAL` or `scripts/ci/projection-check` exits 10 | `src/capability/closure.rs:478`, `:627`; `tests/projections.rs:26`; `src/quality/parity.rs` | P (by design) | 83 executables have no CLI path, no policy says which omissions are deliberate | not placed (SURFACES F-4) |
| CLI (shell) task lifecycle | `bin/majordomus:150`; `usage()` `bin/majordomus:34-88`; `share/commands.yaml`; `lib/<cmd>.sh` | start/check/finish/handover/session/checkpoint/decision as shell | `lib/commands.sh:17-67` reconciles commands.yaml vs dispatch | cases 15, 30, 04, 06 (sub-audit) | A as capabilities | no typed capability for 15 mutating commands (`.ai/repo/development-semantics-baseline.txt:19-33`, ADR 0040) | I2141 |
| Transport parity | `tests/properties.rs:251` | direct = MCP = HTTP(GET) for deterministic executables | refusal class equal over MCP and HTTP | `tests/properties.rs:251`; `tests/http_serve.rs:256`; `tests/mcp_conformance_projection.rs:288`; `tests/product.rs:343` | P | excludes `peers.list`, `peers.announce`, `perf.counters` (`:166-168`); HTTP only for GET (`:300`); no CLI JSON = HTTP test | I2150 |
| Generated docs / registry dataset | `src/generate.rs:36,59-89` | site dataset registry-derived | `generate --check` refuses stale | `tests/projections.rs:320,382,510,633`; `tests/generate_check.rs`; cases 51, 406 | I | second writer `scripts/generate-site-data:285` parses shell usage text (SURFACES F-14) | not placed |
| Native surfaces | hand-written `src/web/discover.rs:150 native_all()`; bound `src/http/router.rs:547-601` | each surface owns its mount | unknown path → 404 naming what is served (`:514-524`) | `tests/http_serve.rs:474`; cases 84, 89 | I (second registry) | outside the capability registry, contradicts ADR 0004 | I2152 |
| Cockpit routes | `src/cockpit/mod.rs:83-111 STATIC_ROUTES` + match `:210-273` + `src/cockpit/nav.rs:119-266 areas()` | 28 static routes 200 (sub-audit) | non-GET → 405 (`src/cockpit/mod.rs:152-158`) | `src/cockpit/mod.rs:400,427,470` (sub-audit); case 760 | I (hand-written, cross-checked) | no areas for sessions, tasks, handovers, reviews, runtimes, events (live 404) | I2155 |
| Cockpit data access | `src/cockpit/pages.rs`, `plan.rs`, `intents.rs`, `mcp.rs` | most pages via `ctx.execute` | unknown is never ok (case 540) | cases 389, 547; `.ai/repo/ui-integrity-baseline.txt` (9 findings) | P | five pages read `ctx.registry` directly (baseline lines 9-13) | I2152 |
| Cockpit actions | browser → capability's `/api/v1` route → `ctx.execute` (`src/http/router.rs:899`) | Cockpit itself GET-only | `share/cockpit/executions.js:28-38` and `plan.js:46-62` confirm | `tests/cockpit.rs:247`; `tests/cockpit_plan.rs` (sub-audit) | P | `share/cockpit/runner.js:103-122` never confirms, even for `repository_mutation` | I2153 |
| Dashboard overview | `src/capability/builtin/dashboard.rs` `CARDS` (from `:476`) | each card = source capability at a JSON pointer | unanswered source → unknown | `tests/dashboard.rs:87`; `dashboard.rs:827,862` | I | card routes hand-written (`:480-612`), test checks prefix only (`tests/dashboard.rs:115-120`); no `observed_at` | I2152, I2149 |
| Freshness metadata | `view::as_of` `src/cockpit/view.rs:524-542`; `repository.info.observed`, `health.report.observed` | `/api/v1/health` has `observed_at`, `source`, `stale_after` | stale badge | case 545; `view.rs` test (sub-audit) | P | dashboard, episodes, lifecycle/runtime, plan/status, served, mesh lack `observed_at`; overview badge permanently stale (`src/index.rs:44`, `src/live.rs:250-252`); `GET /` `stale: null` 12 commits behind HEAD | I2149 |
| Static site honesty | `site/templates/*`, `site/data/*` | mock windows labelled "A picture, not the Cockpit" (`site/templates/screens/cockpit.html:49`) | — | case 104; case 09 (sub-audit) | I | hand-written Cockpit nav (19 vs 24, `site/templates/screens/cockpit.html:22`) while `src/site.rs:1275-1299` exports `cockpit_areas` | I2152 |
| Mobile / a11y / E2E | widths `share/design/tokens.yaml:489`; `scripts/lib/cockpit-probe.mjs`; `scripts/lib/ui-audit.mjs` | overflow per declared width (`cockpit-probe.mjs:555-589`) | — | gates `.ai/repo/ci/gates.yaml:494,506,529,536`; cases 85, 334, 337, 994 (sub-audit) | P | `rust` class (`.ai/repo/ci/gates.yaml:709`) does not select `ui-audit` | I2154 (mobile: mobile-first I2096-I2103) |
| XSS | `src/cockpit/html.rs:21-34 escape`; raw only `src/cockpit/view.rs:65,153`; CSP `src/cockpit/mod.rs:296-310` | escaped by construction | — | `html.rs` 5 tests; `tests/cockpit.rs:853` (sub-audit) | I (Cockpit), P (site) | site `json_encode \| safe` in `<script>` does not escape `</` (`site/templates/partials/graph.html:76`, `site/templates/api.html:94`, `site/templates/why-section.html:284`); exploitability unverified | I2151 |
| Peer board | `src/peers.rs:68,149,217`; server memory | gathered repo-wide with `complete` + `boards[]` | departed peer's claims kept by count, not time (`src/peers.rs:363-367,558-576`; sub-audit) | `src/peers.rs:884-1291`; `tests/peer_claims.rs`; cases 106, 250, 343 | I (observational) | claims gate nothing; excluded from parity | I2143, I2150 |
| Task lifecycle | `lib/start.sh:130` mints `t-…`; `.ai/local/state/current.yaml` | scope enforced at check (`lib/check.sh:142-198`) and finish | `finish --check` exits 0 with no, foreign or non-active task (`lib/finish.sh:41-55`) | cases 04, 06 (asserts the exit 0), 27, 108, 130 | P | pre-push (`.githooks/pre-push:13`) enforces nothing without a task; no TTL or recovery (`lib/recover.sh:63-70`) | I2142, I2143 |
| Mesh claims as coordination | `src/mesh/journal.rs:485 ClaimAcquired`; `src/mesh/state.rs:137-192`; mint `src/mesh/cooperation.rs:3322` | exclusive refuses overlapping exclusive (`src/mesh/state.rs:813-833`) | stream beat expires 30 s | `tests/mesh_cooperation.rs:176,432`; `state.rs:877-1131`; case 368 | P | refuses only another `mesh.claim`; nothing in `lib/` or `.githooks/` reads the mesh; named-session claim auto-opens its session (`src/mesh/cooperation.rs:3319-3321`) and outlives a dead worker | I2142, I2143 |
| Handover | `lib/handover.sh`; `src/mesh/journal.rs:405 HandoverBody`; `src/continuity/record.rs:143-212` | each validates its own shape | sections refused at write (`lib/handover.sh:48-53`) | cases 05, 50, 493; `tests/continuity*.rs`; `tests/mesh_compaction.rs` | D | three stores, no shared id; mesh `task` unvalidated | I2144 |
| Reviews | mesh only `src/mesh/journal.rs:518-546`; `r-<12hex>` `src/mesh/cooperation.rs:3552` | — | — | `tests/mesh_cooperation.rs:176` (sub-audit) | P | no task or claim link; live 0 reviews | I2144 |
| Plan / issue / milestone / intent | `src/plan.rs:342,400`; `src/intent.rs:390`; `.ai/repo/project/*` | `plan.transition` refuses illegal moves (`src/plan.rs:1659-1720`) | ACTIVE never expires | cases 46, 99, 133; `tests/cockpit_plan.rs`; `tests/intent*.rs` | I (two engines held equal by case 99) | plan writes other than `plan.transition` shell-only | I1504 (existing pattern), I2141 |
| GitHub sync | `scripts/github-sync` | `--check` exits 11 on drift | baselines `missing 192`, `adopt 19` (`.ai/repo/ci/github-drift-baseline.txt`) | cases 45, 97, 894 | P | GitHub numbers never stored locally; mesh `issue` accepts `#N` or `I####` unmapped (`src/mesh/journal.rs:306,620-628`) | I2144 (mesh half); GitHub numbers not placed |

</div>


### 1.5 Failure handling, diagnostics, enforcement, update and release

<div class="overflow-x-auto" tabindex="0">

| capability | canonical owner | healthy path | fault path | test | status | gap | issue id |
|---|---|---|---|---|---|---|---|
| Liveness | `src/capability/builtin/health.rs:202` (`GET /api/v1/live`) | alive + version + commit | n/a | `health.rs` `liveness_is_the_same_answer_whatever_the_layer_holds` (`:818`) | I | — | — |
| Readiness | `src/capability/builtin/health.rs:214` (`GET /api/v1/ready`) | registry > 0 and not replaced code | stale → `ready:false`; degraded layer → `layer: warn` but ready | `readiness_reports_what_this_process_already_holds` (`:841`); `tests/health_server.rs` | P | degraded index stays ready; mesh and lease loss absent; `was_lost()` only on `GET /` | I2157 |
| Health report | `src/capability/builtin/health.rs:299` | worst of checks | unknown ≠ ok (`:101`) | `tests/health_server.rs` (4); unit tests `:775-871` | P | no mesh or policy-allowed dimension | I2157 |
| Running vs ready vs mesh vs degraded | `/live`, `/ready`, `server.standing_at` (`src/capability/builtin/server.rs:403`), `MeshStatus`, `index::State::Degraded` (`src/index.rs:34`), `LinkState::Degraded` (`src/mesh/cooperation.rs:265`) | — | kept apart (`src/capability/builtin/health.rs:19-36`) | per module | P | no typed state machine joins them; degraded has three meanings | I2130, I2157 |
| Governance doctor | `lib/doctor.sh:53` | exit 0/10/12 | policy-parse failure short-circuits (`:65`) | doctor cases (unverified individually) | I | checks nothing about server, lease or mesh; name suggests runtime health | I2157 |
| Mesh doctor | `src/mesh/doctor.rs:93,138` | ordered checks | fail with impact + remediation (`:70`) | `doctor.rs` tests `:520-683` | I | cannot see beat liveness (by design) | — |
| Structured logging | `src/logging.rs:29` | stderr only | double init is a no-op | 2 unit tests | P | human fmt, no JSON, no correlation id, no redaction; request paths/origins logged raw (`src/http/router.rs:731-736`) | I2158, I2161 |
| Secret redaction (text) | `src/redaction.rs:113 redact_secrets`, `:223 public_text` | ported from `lib/capture.sh` with a parity fixture | refuses unredactable armour | `tests/redaction.rs`; case 504 | P / M | `public_text` has no production caller; module doc and case 504 say evidence goes through it | I1955 (caller), I2161 (doc) |
| Secret redaction (execution input) | `src/execution/redact.rs:53` (`x-majordomus-sensitive`) | redact before store | — | `redact.rs:118` | P | no capability declares a sensitive field; rule names `tests/executions.rs`, which has no secret test | I2160 |
| Published-artifact guard | `src/generate.rs:1364 FORBIDDEN`, `:1398 forbidden_in` | refuse to write | yes | `src/generate.rs:3105-3115` | P | substring list; not applied to HTTP, MCP, frames or logs | I2161 |
| HTTP state-change guard | `src/http/router.rs:503 handle` → `:718 foreign_origin` | browser cross-origin POST refused | yes for browsers | `tests/cockpit.rs:747`; `router.rs:1025` | P | no authentication for non-browser clients beyond loopback | I2123 |
| Non-loopback bind warning | `src/http/server.rs:32-47` | warn | warn only | `tests/mcp_shared.rs:941`; `tests/http_host.rs:14` | I (M) | text says hosts "can read"; they could also write | I2123 |
| Mesh link admission | `src/mesh/cooperation.rs:1765`; `src/mesh/link.rs:50-76` | signature, skew, nonce, protocol, repo, trust | typed `RefusalCode` | `cooperation.rs:3766`; `tests/mesh_cooperation.rs:592`; `link.rs:944` | I | only link routes authenticate | I2123 |
| Bounded waits / retries | `src/lease.rs:43-62`; `src/mesh/link.rs:76`; `src/mesh/cooperation.rs:2611` | bounded client-side | yes | `tests/lease_busy.rs:92-245` | P | no server-side read or handler timeout; 4 workers read bodies unbounded in time (`src/http/server.rs:208-222`) | I2156 |
| Bench regression budget | `src/bench/baseline.rs:1-8`; `.ai/repo/benchmarks/rust/policy.yaml`; macOS baseline only | gate `rust-bench` (`.ai/repo/ci/gates.yaml:323`) | other platform not compared | case 81 | P | bench job cancelled in each of the last 10 scheduled runs | I2159 |
| Version mismatch, server vs new binary | `src/lease.rs:695` (same path); `src/capability/builtin/server.rs:410-424` | same-path replacement taken over | other-path version → Outdated warning only | `lease.rs:1253,1271`; cases 33, 107 | P | `elect` attaches a new client to an older server; MCP clients never learn | I2163 |
| Mesh wire versioning | discovery `PROTOCOL_VERSION=2`/`MIN=1` (`src/mesh/protocol.rs:36-40`); link `1..1` (`src/mesh/link.rs:50-53`) | range check, negotiation | `Refusal::Version` | `protocol.rs:353`; `link.rs:944` | P | no v1 fixture test; no gate holds a wire change to a version bump | I2164 |
| Public contract versioning | `src/release/compat.rs`, `src/release/surface.rs:65`; gate `version-surface` | surface diff → required bump | unknown → major | `tests/cli.rs:392,569`; case 743 | I | covers capabilities, routes, tools, commands, schemas only | I2164 |
| Installer / update | `site/static/install.sh` (776 lines; live copy byte-identical) | resolve → verify sha256 + size → stage → rename | refuses mismatch, traversal, non-HTTPS | case 85; `scripts/ci/install-check` | I | no signature; no self-update command | I2165 |
| Release pipeline | `.github/workflows/release.yml` (plan → build → publish → smoke) | `release-verdict` (`:76`), `release-verify` (`:131-137`) | adopts an existing release on rerun | cases 87, 87b, 362 | I | smoke checks version + init only; `majordomus-mcp --help … \|\| true` (`:452`); no server started from the installed release | I2162 |
| Full CI plan verdict | `.github/workflows/validate.yml` scheduled runs; `scripts/ci/verdict` | every gate selected | — | — | P | last 10 scheduled runs cancelled/failed (2026-10-09: bench and macos cancelled, suite failed on case 95 at 3495 s vs 3450 s); push runs green on a subset; nothing alarms | I2159 (I2115 for case 95) |
| Rule proof | `scripts/ci/rule-proof-check` | proof named and present | — | — | P | 163 rules, 137 blocking, 133 not run, 3 recorded passing; "satisfied" ≠ ran | I2160 |
| Dogfood adopter with a real server | suite cases in disposable `init` repos with the checkout's binary; `scripts/ci/install-check:176` runs `--standalone` against this repo | — | — | case 90; `tests/external_extension.rs` | P | absent for an installed release serving a foreign repo | I2162 |

</div>


### 1.6 Failure classes

<div class="overflow-x-auto" tabindex="0">

| failure | handled (file:line) | test | status | issue id |
|---|---|---|---|---|
| SIGKILL during lease write | empty file taken over after `BIND_GRACE` (`src/lease.rs:720-726`); publish tmp+rename (`:1095-1101`) | `tests/mcp_shared.rs:848`; case 113 | I (kill between `create_new` and `write_all` untested) | I2158 |
| Disk full | lease publish → `Error::io` → standalone (`docs/MCP.md:56`); journal ignores errors (`src/mesh/journal.rs:1984`); keep-alive ignores (`src/lease.rs:1059`); ledger bare `>>` (`lib/common.sh:893`) | none (case 276 is about reclaiming space) | P | I2137, I2158 |
| Read-only state directory | election fails → standalone | `tests/mcp_shared.rs:903` | I (lease only; journal unverified) | I2137 |
| Truncated / corrupt journal | unparsable lines dropped silently (`src/mesh/journal.rs:1408-1413`); torn tail fuses (`:1969-1984`) | clean reload only (`journal.rs:2655`) | P | I2137 |
| Partial write | lease atomic rename; journal non-atomic, no fsync; only `src/economics/runner.rs:1035` syncs | none for the journal | P | I2137, I2128 |
| Port collision | `bind_or_fallback` (`src/http/server.rs:77`) | `tests/mcp_shared.rs:627` | I | — |
| Compatibility mismatch | same-path supersede; version → Outdated; continuity future schema refused; mesh version refusal | `lease.rs:1253`; `tests/continuity.rs:242`; `protocol.rs:353`; `tests/mcp_shared.rs:848` | P (cross-path version attach) | I2163 |
| Clock skew | mesh ±300 s (`src/mesh/protocol.rs:48`, `src/mesh/link.rs:73`); liveness monotonic (`src/mesh/journal.rs:1987-1991`) | `protocol.rs:362`; `journal.rs:2545`; `tests/mesh_cooperation.rs:592` | I | — |
| Slow peer / slow client | slow owner waited (`src/lease.rs:43-62`); link 5 s timeout | `tests/lease_busy.rs:92,177,223` | P (no server-side timeout, no slow-client test) | I2156 |
| Last client leaving | server lingers, then stops and removes the lease (`src/shared.rs:228,266`) | `tests/mcp_shared.rs:450`; case 90 | I | — |
| Frozen process | silent owner loses lease after `BUSY_GRACE` (`src/lease.rs:54`); on thaw `lost()` (`:820`) | `tests/lease_busy.rs:131`; `tests/lease_lost.rs:73` | I (frozen worker in a live server absent) | I2158 |

</div>


### 1.7 Enforcement

<div class="overflow-x-auto" tabindex="0">

| invariant | gate (file:line) | runs in CI | status | issue id |
|---|---|---|---|---|
| No shadow operation inventory | `projection-closure` → `scripts/ci/projection-check`; `command-graph`; `mcp-tool-run` (`.ai/repo/ci/gates.yaml:295`); `shell-inventory` (`:473`); `mesh-check` | yes, path-selected | I | — |
| No unauthenticated remote mutation | link routes only (`scripts/ci/mesh-check:70`, `tests/mesh_cooperation.rs:592`); other POST routes Origin only (`src/http/router.rs:718`) | partial | A for non-link routes | I2123 |
| No unknown remote auto-trust | `DenyUnknown` default (`src/mesh/trust.rs`, `src/mesh/config.rs:397`); this repo `policy: deny_unknown`; no gate refuses committed `tofu` | unit tests only | P | I2135 |
| No secrets in responses | `prompt-privacy` (`.ai/repo/ci/gates.yaml:39`), `no-machine-paths` (`core-check:81`), `forbidden_in` | yes | P (responses, logs, frames unscanned) | I2161 |
| No unversioned wire change | `version-surface` | yes, path-selected | P (mesh envelopes, lease schema, journal events uncovered) | I2164 |
| Docs drift | `doc-command-check`, `reference-check`, `tests/cli_docs.rs`, `command-furnished` | yes | I (prose claims unchecked) | — |
| Generated drift | `generate --check` (`scripts/rust-check:312`), `generation-converges`, pre-commit `scripts/pages current` | yes | I | — |
| No false green after skips | `test/run.sh --no-skips` (`:224,342-355`, `validate.yml:286`); `scripts/ci/verdict` reports skipped gates; zero `#[ignore]` | yes | P (full plan red 10+ days, nothing alarms) | I2159 |
| Rule proof typed | `rule-proof` | yes, path-selected | P (satisfied ≠ ran) | I2160 |
| Pre-push | `.githooks/pre-push` → `finish --check` only | local | P | I2142, I1990 |
| Release follows CI | `scripts/ci/release-verdict` (`release.yml:76`) | yes | I | — |
| Dogfood installed release | `scripts/ci/install-check:176` (`--standalone`, this repo) | partial | A for a foreign repo over the shared server | I2162 |

</div>


## 2. Consistency contract of the mesh, as implemented

<div class="overflow-x-auto" tabindex="0">

| property | what holds (file:line) | issue id |
|---|---|---|
| Immutable | a stored event, identified by `(stream, seq)` and signed (`src/mesh/journal.rs:934-948`); first copy per `(stream, seq)` wins per runtime (`:1929`); equivocation not detected | not placed |
| Durable across restart | node key; every accepted event in `.ai/local/state/mesh/journal.jsonl` incl. other runtimes' events and handover bodies (`src/mesh/journal.rs:1961-1985`); no fsync, ignored write errors | I2137 |
| Memory only | registry, link table, nonce cache, refusal list, tombstones, beats, link counters (`src/mesh/registry.rs:7-9`, `src/mesh/cooperation.rs:311-318`, `src/mesh/journal.rs:1292-1298`) | — |
| Eventual | replication by mark comparison, at-least-once, idempotent (`src/mesh/journal.rs:1734-1766,1929-1935`); digest equal only for equal event sets and liveness verdicts (`src/mesh/state.rs:502-505`) | I2138 (proptest) |
| Order | within a stream by seq (gaps buffered ≤256, ≤4096 total, `src/mesh/journal.rs:1936-1943`); across streams `(lamport, stream, seq)` (`src/mesh/state.rs:508-513`); lamport max+1, unbounded and unvalidated (`src/mesh/journal.rs:1473,1953`) | I2124 |
| Liveness | stream Live while its signed beat rose within `expiry_seconds` on the local monotonic clock (`src/mesh/journal.rs:2012-2021`); links are a separate layer (`src/mesh/cooperation.rs:2644-2658`) | — |
| Expiry | fold-time verdict, reverses if the same stream beats again; revived claim wins by lamport; a restart is a new stream, so no revival from the own restart | I2138 |
| Resurrection from disk | not by own restart; a trusted relay can make a reloaded foreign stream Live for up to one expiry (unsigned `age_ms`, `src/mesh/journal.rs:1689-1696`) | not placed |
| Exclusivity | local admission + deterministic post-hoc naming; no fencing token, no epoch, no notification (`src/mesh/state.rs:684-703`); linearizability disclaimed (`docs/MESH.md:26-27,612-613`) | I2138, I2136 |

</div>


## 3. Lifecycle and connection state models

Pack M01 asks for a service state machine (stopped, starting, ready, degraded, stopping, failed)
and a connection state machine (candidate, authenticated, connected, degraded, expired, refused)
built from existing types. What exists:

<div class="overflow-x-auto" tabindex="0">

| model | existing type (file:line) | states today | missing | issue id |
|---|---|---|---|---|
| Service | `ServerStanding` (`src/capability/builtin/server.rs:163-177`), decided by `standing_of` (`:362-425`) | absent, starting, ready, outdated, stale | degraded, stopping, failed; it is an observer's reading of the lease, not server-owned | I2130 |
| Readiness | `Readiness` (`src/capability/builtin/health.rs:214`) | ready true/false (stale code only) | lease lost, degraded index, mesh | I2157 |
| Connection (link) | `LinkState` (`src/mesh/cooperation.rs:265`, supervised `:2513-2658`) | connected, degraded, unreachable, expired | candidate and refused are separate: registry `Observed` (`src/mesh/registry.rs`) and typed `RefusalCode` lists (`src/mesh/cooperation.rs:1765-1953`) | not placed (naming only) |
| Discovery trust | `Trust` (`src/mesh/trust.rs:92-116`) | trusted, observed | revoked | I2134 |

</div>


## 4. Threats (mesh and HTTP)

<div class="overflow-x-auto" tabindex="0">

| threat | status | evidence | issue id |
|---|---|---|---|
| T1 UDP amplification | defended | providers never answer a datagram (`src/mesh/multicast.rs:228-256`, `src/mesh/broadcast.rs:175-200`) | — |
| T2 HTTP register amplification | open (low) | ~1 KB garbage → own envelope + ≤32 candidates (`src/mesh/manager.rs:623-636`) | I2133 |
| T3 Datagram flood | open | unbounded channel to one verify thread (`src/mesh/manager.rs:425,467-478`) | I2133 |
| T4 Oversized replies | open | `read_to_end`, per-read timeout only (`src/mcp/bridge.rs:71,84`; `src/mesh/cooperation.rs:2383-2387`) | I2133, I2147 |
| T5 Advertisement replay | defended per instance | `src/mesh/registry.rs:252-260` | — |
| T6 Hello replay | open across runtimes | no audience (`src/mesh/link.rs:444-456`) | I2132 |
| T7 Sync replay | defended | rising counter + signature (`src/mesh/cooperation.rs:2041-2070`) | — |
| T8 Forged origin on the wire | defended | `src/mesh/journal.rs:1857-1866`, beats `:1652-1659` | — |
| T9 Signing oracle over HTTP | open at 1fb8d6be44 | `POST /api/v1/mesh/claims` with any session (`src/capability/builtin/mesh.rs:787-806,200-215`) | I2123 (done on branch) |
| T10 Unknown key after reload | open on reload | `src/mesh/journal.rs:1419` | I2124 |
| T11 Revocation / rotation | absent | `src/mesh/manager.rs:466`, `src/mesh/cooperation.rs:1150` | I2134 |
| T12 Trust root in branch content | open | `src/shared.rs:34-48` | I2135 |
| T13 Cross-repo leakage | partial | repo id in clear; `mesh.register` returns every repo's candidates (`src/mesh/registry.rs:342-357`) | I2133 |
| T14 Plaintext HTTP | open by design | `src/mesh/link.rs:878-908`; ADR 0067 alternatives | non-scope of runtime-trust (ADR 0032) |
| T15 Address substitution | defended for signed data | seeds unsigned config; dialer does not check the welcoming runtime (`src/mesh/cooperation.rs:2576-2578`), benign | — |
| T16 Downgrade | defended | link 1..1 (`src/mesh/link.rs:50-53,217-221`) | — |
| T17 Lamport manipulation | open | `src/mesh/journal.rs:1952-1953` | I2124 |
| T18 Clock skew | bounded | ±300 s (`src/mesh/protocol.rs:259`, `src/mesh/cooperation.rs:1797,2231`) | — |
| Swagger on the write origin | open | no CSP/SRI (`src/http/swagger.rs:52,79`) | I2151 |
| DNS rebinding for GET | open (unverified exploit) | only non-GET checks Host (`src/http/router.rs:498-504,749`) | I2151 |

</div>


## 5. Drift between documents and code

### 5.1 Lifecycle and MCP

1. Declared lease timings are "enforced" (`docs/MCP.md:284-303`, `.ai/repo/policy.yaml:335-339`); not in `mcp`/`serve` (`src/lease.rs:191-193,1043`, `src/commands/mcp.rs:173-174`, `src/capability/builtin/server.rs:370,386`, `src/commands/serve.rs:330,358`). → I2129
2. "Writes one file: the lease" (`docs/MCP.md:33-36`, `src/lease.rs:9-11`, `README.md:542-543`); it also writes the journal (`src/shared.rs:119`), `server.log`, episodes, executions, and six tools write the tracked tree. → I2130
3. "Read-only … one shared server per repository … ends when its last client leaves" (`README.md:405-408,539`); six writers, one server per checkout (`src/lease.rs:13-16`, ADR 0044), a `serve ensure` server lives 900 s with no client (`src/cli.rs:3427`). → I2130
4. ADR 0003 (accepted) says Swagger at `/docs` (`.ai/repo/adrs/0003-*.md:40`); code serves `/swagger`. Its "no kind writes" and "no process without a client" are superseded in code by ADRs 0040 and 0043, both proposed. → I2152
5. `SharedServer::stop` comment (`src/shared.rs:274-281`) implies `serve stop` runs it; it does not (`src/commands/serve.rs:814,890-903`, `src/lease.rs:1197-1211`). → I2128
6. `LeaseView.pid` "informational" (`src/capability/builtin/server.rs:236`) vs `src/lease.rs:742`, `src/commands/serve.rs:354-356,814`. → I2128
7. `docs/MCP.md:48` (ping) and `:50` (ends when last peer goes) make a superseded server immortal; `:118` does not say so. → I2127
8. MCP initialize instructions "only [six tools] write" (`src/mcp/protocol.rs:426-433`); `majordomus_execution_start` can execute them (`src/execution/engine.rs:206-223`). → I2123 for remote callers; local not placed (INDEX)
9. `src/http/router.rs:498-502` "a read is already contained" ignores DNS rebinding for GET. → I2151
10. `docs/MCP.md:29` shows `:8741` as the norm; second checkouts get a random port. → I2131
11. `docs/DEVELOPMENT_RUNTIME.md:321` lists only case 108 as the shared-server test. → I2158
12. Pack premise "served.rs" is lifecycle: **refuted**, `src/served.rs:1-37` is deployment observation; case 856 is the integration lease (ADR 0101). → no issue

### 5.2 Mesh

1. Handshake check order: `docs/MESH.md:128-136` and the rule put the nonce first; code checks it last (`src/mesh/cooperation.rs:1847-1875`). → I2132
2. Unauthenticated write surface absent from `docs/MESH.md:330-356` and claim `mesh-observation-not-authority`; `docs/MESH.md:437,570` tell operators to bind `0.0.0.0`. → I2123, I2136
3. "An exclusive claim made anywhere refuses an overlapping claim everywhere" (`docs/MESH.md:21-22`, rule rationale, claim `mesh-claims-cross-runtime`); admission is local (`src/mesh/cooperation.rs:3310-3318`). → I2136
4. Expiry "everywhere on its own" (`docs/MESH.md:229-230`, `src/capability/builtin/mesh.rs:1271`); it reverses (`src/mesh/journal.rs:1681-1697`, `src/mesh/state.rs:656-661`). → I2138
5. "Messages (900 KiB)" as a flood bound (`docs/MESH.md:341`); replies to the dialer and rendezvous answers unbounded (`src/mcp/bridge.rs:84-85`). → I2133
6. TOFU key-change rejection (`src/mesh/trust.rs:31-33`) is unreachable (`src/mesh/identity.rs:40-45`). → I2136
7. ADR 0050 stale: kind `mesh` vs `mesh-declaration` (`src/mesh/config.rs:28`); "trusted never evicted" vs allowlisted only (`src/mesh/registry.rs:295`); "nothing persists but the identity" vs the journal (`src/shared.rs:119`); "mode 0600" vs chmod after write (`src/mesh/identity.rs:339-345`). → I2136
8. ADRs 0050, 0059, 0067 `status: proposed` while their rule is active and blocking. → I2136
9. `EventList.lamport` cursor doc (`src/capability/builtin/mesh.rs:594-596`) vs `src/mesh/journal.rs:2124-2133`. → I2140
10. "Only the holder's current run releases" (`docs/MESH.md:229`) is true per run, not per session (`src/mesh/cooperation.rs:3373-3390`). → I2138
11. "A property test permutes and duplicates deliveries" (`docs/MESH.md:189-190`): one stream of six events (`src/mesh/journal.rs:3333-3343`), one fixed history (`src/mesh/state.rs:1105-1126`). → I2138
12. Declaration "committed" (`docs/MESH.md:365,382-399`, ADR 0050 rule 6); the working tree is read (`src/shared.rs:34-48`). → I2135

### 5.3 Projections, Cockpit, coordination

1. ADR 0004 "nothing written a second time", "no second registry", "every call through `Context::execute`" (`.ai/repo/adrs/0004-*.md:41-43,52,57-61`) vs clap (`src/capability/closure.rs:6-9`), `src/cli/local.rs:137`, `src/web/discover.rs:150`, `share/commands.yaml`. → I2152
2. ADR 0004 `command` = "changes this process's memory" (`:37`) vs six `repository_mutation` capabilities. → I2152
3. `src/cli/local.rs:52-55` "every projection is read-only" vs the six writers. → I2152
4. CLAUDE.md "what refuses a change is the task's own scope, at check, at finish and in the pre-push hook" vs `lib/finish.sh:41-55` (exit 0 with no task). → I2142
5. CLAUDE.md ties `check --overlap` to the board; `lib/start.sh:171-184`, `lib/check.sh:283-287` compare local `current.yaml` only. → I2142
6. `lib/check.sh:75-77` "the record is tracked"; it is under untracked `.ai/local/state/` (`lib/start.sh:61-63`, `.gitignore:62`). → I2142
7. `lib/check.sh:241-244` open questions "tracked"; store is `.ai/local/state/open-questions.md` (`lib/question.sh:6`). → I2144
8. `docs/MESH.md:316` sessions share intent, issue, task, milestone; the board sets none (`src/mesh/cooperation.rs:1345-1357`). → I2144
9. `docs/MESH.md:18-24` and CLAUDE.md imply a claim dies with its worker; a named-session claim survives while the runtime beats (`src/mesh/cooperation.rs:3319-3321`). → I2143
10. `docs/MESH.md:309-313` a detached peer's claims are released; the local board keeps them (`src/peers.rs:540-576`). → I2143
11. `docs/CONTINUITY.md:81-82` vs `lib/session.sh:440,466,676` on tasks per record. → I2144
12. `docs/SURFACES.md` F-5 (and 554, 695-696, 719) "no Cockpit area for the peer board"; `/cockpit/peers` exists (`src/cockpit/mod.rs:96`, `src/cockpit/nav.rs:188-190`). → I2155
13. `docs/SURFACES.md` F-12 "HTTP = MCP for a sample"; `tests/properties.rs:251` compares every deterministic executable; the CLI half stands. → I2150
14. `docs/COCKPIT.md:50,75,82,405-413,516-519`, `docs/SURFACES.md:319` understate data sources, JS list, "add a page" steps (sub-audit). → I2152
15. `src/cockpit/pages.rs:4-8` "every page goes through `Context::execute`"; five do not (`.ai/repo/ui-integrity-baseline.txt:9-13`). → I2152
16. `src/cockpit/nav.rs:11-13` lists 16 areas; `areas()` has 24 (sub-audit). → I2152
17. `docs/HARDCODING_LEDGER.yaml` rows `command-surface-has-no-owner` (half-resolved) and `site-tile-route-list` (cites `scripts/site-check:88`, now `:840`). → I2152
18. `scripts/lib/cockpit-probe.mjs:18` "three widths"; code uses five. → I2154
19. Live `GET /` `stale: null` while serving commit 893a79be6577, 12 behind HEAD. → I2149

### 5.4 Failure, enforcement, update

1. `src/redaction.rs:1-7` and case 504 header: published evidence goes through `public_text`; nothing calls it. → I2161 (doc), I1955 (caller)
2. `src/http/server.rs:47` and `docs/MCP.md:225-262`: beyond loopback hosts "read"; they could POST writers (`src/http/router.rs:503-508`). → I2123
3. `docs/MESH.md:605` "authenticated links" true for link routes; other mesh POST routes made this node sign. → I2123
4. `src/mesh/protocol.rs:33-35` "a version-2 reader reads both"; no v1 test. → I2164
5. Rule `project.executions-carry-no-secret` names `tests/executions.rs`; no redaction test there. → I2160
6. `src/logging.rs:3` "structured" vs human fmt (`:31-36`). → I2158
7. `docs/MCP.md:49` superseded = "yesterday's code"; holds only for the same path (`src/lease.rs:690-701`). → I2163
8. Pack premise "`majordomus update` is self-update" vs `lib/update.sh:4`, `docs/DISTRIBUTION.md:416`. → refuted; I2165 records it
9. `docs/DISTRIBUTION.md` smoke "installing the release" vs `release.yml:452` `|| true`. → I2162
10. `.ai/repo/ci/gates.yaml:327` `rust-bench` "against the committed baseline"; no verdict in 10 scheduled runs. → I2159

## 6. Premises of the pack the code refutes

Each is **REFUTED** at 1fb8d6be44. The milestone named carries the correction.

<div class="overflow-x-auto" tabindex="0">

| # | pack premise | what the code shows | record |
|---|---|---|---|
| R1 | **REFUTED**: `majordomus update` is self-update | it regenerates provider projections (`lib/update.sh:4`); no self-update exists (`docs/DISTRIBUTION.md:416-439`); upgrade = rerun `install.sh --version` | safe-self-update, I2165 |
| R2 | **REFUTED**: releases are signed | checksummed only: sha256 + size in `releases/<tag>.json` checked by `site/static/install.sh:478` (called `:706`); signing deferred (`docs/DISTRIBUTION.md:151-153`) | safe-self-update, I2165 |
| R3 | **REFUTED**: MCP keeps operation lists independent of the registry | tools and resources are a registry walk (`src/mcp/surface.rs:367-394`), annotations from effect (`src/mcp/protocol.rs:446-456`); held by `tests/projections.rs:26`, case 940 | mcp-transports-are-reliable |
| R4 | **REFUTED**: there is a `majordomus ui` | no such command; `serve ensure` and `env enter` are the UI-first entries (`src/commands/serve.rs:481-576`, `src/commands/env.rs:701-760`) | runtime-lifecycle, I2131 |
| R5 | **REFUTED**: the Cockpit takes actions | Cockpit routes are GET-only (`src/cockpit/mod.rs:152-158`); the browser calls each capability's `/api/v1` route; the one gap is the runner's missing confirmation | cockpit-control-center, I2153 |
| R6 | **REFUTED**: "superseded lease" covers an upgrade | only a binary replaced at the same path (`src/lease.rs:695-709`); installer versions live in separate directories | safe-self-update, I2163 |
| R7 | **REFUTED**: top-level doctor checks runtime health | `lib/doctor.sh:53-77` is a governance doctor | failure-is-visible, I2157 |
| R8 | **REFUTED**: logging is structured | human `tracing` fmt on stderr, no JSON, correlation id or redaction (`src/logging.rs:29-37`) | failure-is-visible, I2158 |
| R9 | **REFUTED**: `served.rs` is server lifecycle | deployment observation of `/build.json` (`src/served.rs:1-37`) | runtime-lifecycle (current_state) |
| R10 | **REFUTED** in framing: the mesh offers exclusive claims | local admission + post-hoc naming; linearizability disclaimed (`docs/MESH.md:26-27,612-613`) but overstated at `docs/MESH.md:21-22` | mesh-journal-converges, I2136, I2138 |
| R11 | **REFUTED**: CI is green | push CI on master is green on a subset; the full scheduled plan has been cancelled or failed for its last 10 runs | architecture-is-enforced, I2159 |

</div>


Premises that hold: Ed25519 identities, signed journal, multicast/broadcast/rendezvous (IPv4 only),
deny-unknown default, mesh off by default, eventual claim conflict under partition, plain HTTP
without confidentiality (and, at 1fb8d6be44, with an unauthenticated write surface, I2123).

## 7. Dependency-aware migration plan

Order follows blast radius, then the pack's mission order. Issues inside one milestone that share
hot files (`src/lease.rs`, `src/shared.rs`, `src/commands/serve.rs`, `src/mesh/cooperation.rs`,
`src/mesh/journal.rs`) are marked not parallel-safe and should be carried by one worker in order.

1. **Security and data loss first (p0).** I2123 (done on branch) and I2124 (in progress); then
   I2145 (no double execution), I2137 (journal durability, after I2124), I2135 (trust root),
   I2134 (revocation and rotation), I2151 (Swagger CSP, Host check).
2. **Lifecycle (runtime-lifecycle).** I2128 → I2130; I2126, I2127, I2129, I2131 independent of
   each other but not parallel-safe. I2059 (existing) removes the launcher-build supersession.
3. **Mesh convergence.** I2138 (fencing; needs I2164's versioning for the wire change), I2139,
   I2140; trust follow-ups I2132 (needs I2164), I2133 (after I2147), I2136.
4. **Transports.** I2147 → I2133; I2146, I2148.
5. **Coordination.** I2141 → I2155; I2142, I2143, I2144.
6. **Surfaces and Cockpit.** I2149, I2150, I2152; I2153, I2154.
7. **Failure visibility.** I2156, I2157, I2158.
8. **Enforcement.** I2159, I2160, I2161, I2162.
9. **Compatibility.** I2164 early (it gates wire changes in steps 3); I2163 after I2127; I2165 last.

Milestone `depends_on` is left empty in every record written here: the real cross-milestone edges
are issue edges (I2133 → I2147, I2155 → I2141, I2163 → I2127, I2130 → I2128), and an edge to
runtime-trust or I2124 would fail `plan validate` on master until that branch lands.
{% endraw %}
