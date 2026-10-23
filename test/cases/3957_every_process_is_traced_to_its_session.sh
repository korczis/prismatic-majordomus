# majordomus-covers: capability:peers.list capability:mesh.who
# Every worker, and every process it starts, is traced to its session (I3956).
#
# On 2026-10-22 a 33-minute remote `cargo test --doc` could be traced to its Claude Code
# session only by hand: ssh client, parent chain, provider pid, its messaging socket, and a
# start time matched against the session list. The board row named a client and a version.
#
# Slice 1, the board row carries the address that reaches the worker, derived from the
# environment the provider hands the MCP server it starts, never asked of the model:
#
#   the server's own stdio client     located from the server process's environment
#   a bridged client                  located from the bridge's environment, carried in the
#                                     initialize it forwards
#   a client exposing no provider     still named by its machine and its process
#   the messaging token               never on the board
#
# Slice 2, a remote run carries its owner and stops as a group, `majordomus offload` driven
# end to end against a host that is a directory here, reached through a stand-in for ssh
# (MAJORDOMUS_OFFLOAD_SSH; nothing leaves this machine):
#
#   the owner                         session and pid from the address model, issue from
#                                     the active task, exported as MAJORDOMUS_OWNER_* into
#                                     the derive, and never the messaging token
#   the derive                        the leader of a process group of its own (setsid)
#   the run record                    beside the queue while the run lives, naming the
#                                     owner, the group, the tag, the start and the cwd;
#                                     gone when the run ends
#   offload stop                      refused to another session, nothing signalled; the
#                                     owner's stop ends the whole group, a grandchild that
#                                     would have outlived its parent included
#
# Slice 3, `majordomus mesh who` names the session behind a subject from the facts alone —
# a process table, a provider's session registry, the run records — through stand-ins for
# ps, lsof and ssh (nothing is signalled, nothing leaves this machine):
#
#   a worker's process                the session from its environment, its name, socket and
#                                     pid from the provider's registry entry whose start
#                                     matches; an entry from an earlier process is not used
#   a leftover                        init as its parent, its directory gone: marked, and
#                                     named with its owner by the run record of its group
#   host:pid, host:tag                a declared host read through the offload ssh path only;
#                                     an undeclared one is refused and nothing is reached
#   an issue, a path                  the run records that name it or work in it
#   unknown                           no owner evidence: exit 10, every missing fact listed
#   MCP                               the same answer as a tools/call
#
# Slice 4, a raw ssh build on a cluster host is refused before it runs: Claude Code's
# PreToolUse payload for the Bash tool piped into `majordomus guard hook` and into the tracked
# shim, the hosts read from this checkout's offload declaration (ssh is a stand-in that logs;
# nothing is run):
#
#   refused, exit 2                   cargo, rustc, scripts/derive, scripts/rust-* over ssh on
#                                     the declared host — options, quotes, env and nice
#                                     prefixes, `--`, bash -c on either side, user@ and a
#                                     tailnet name — naming the rule and `offload derive` /
#                                     `offload stop` as the way
#   let through, exit 0               a read (ps, df, uptime), `majordomus offload` on the
#                                     host, the offload command itself, a local build, and
#                                     any build on a host no declaration names
#
# Real processes over real MCP, an ephemeral port, the server ended with its last client.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || skip "jq not installed"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_LOG=info; export MAJORDOMUS_LOG

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-3957.XXXXXX")"
held=""
trap 'exec 3>&- 2>/dev/null; [ -n "$held" ] && kill "$held" 2>/dev/null; rm -rf "$S"' EXIT

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

req() { printf '{"jsonrpc":"2.0","id":%s,"method":"%s"%s}\n' "$1" "$2" "${3:+,\"params\":$3}"; }
init() { printf '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"%s","version":"0"}}' "$1"; }
# a process with no provider in its environment, whatever runs this case
bare() {
  env -u CLAUDECODE -u CLAUDE_CODE_SESSION_ID -u CLAUDE_CODE_MESSAGING_SOCKET \
    -u CLAUDE_CODE_MESSAGING_TOKEN -u CLAUDE_PID -u MAJORDOMUS_OWNER_SESSION \
    -u MAJORDOMUS_OWNER_PEER -u MAJORDOMUS_OWNER_ISSUE -u MAJORDOMUS_OWNER_HOST \
    -u MAJORDOMUS_OWNER_PID "$@"
}
TOKEN="tok-3957-never-on-the-board"

# ---------------------------------------------------------------- 1. the server's own client
mkfifo "$S/in1"
bare env CLAUDECODE=1 CLAUDE_CODE_SESSION_ID=sess-a CLAUDE_CODE_MESSAGING_SOCKET="$S/a.sock" \
  CLAUDE_PID=4242 CLAUDE_CODE_MESSAGING_TOKEN="$TOKEN" \
  "$RB" mcp --http-port 0 < "$S/in1" > "$S/out1.txt" 2> "$S/err1.txt" & p1=$!
exec 3> "$S/in1"
i=0
until grep -q 'listening on http://' "$S/err1.txt" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 300 ] || { echo "    the first client's server never listened"; cat "$S/err1.txt"; exit 1; }
  sleep 0.1
done
req 1 initialize "$(init case3957-a)" >&3

# ---------------------------------------------------------------- 2. a bridged client
{
  req 1 initialize "$(init case3957-b)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_announce","arguments":{"intent":"a remote derive","scope":["apps"]}}'
  req 3 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}'
} > "$S/in2"
rc=0
bare env CLAUDECODE=1 CLAUDE_CODE_SESSION_ID=sess-b CLAUDE_CODE_MESSAGING_SOCKET="$S/b.sock" \
  CLAUDE_PID=4343 CLAUDE_CODE_MESSAGING_TOKEN="$TOKEN" \
  "$RB" mcp < "$S/in2" > "$S/out2.txt" 2> "$S/err2.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the bridged client exited $rc"; cat "$S/err2.txt"; exit 1; }
grep -q 'bridging this stdio session to it' "$S/err2.txt" || { echo "    the second client did not bridge"; cat "$S/err2.txt"; exit 1; }
board="$(sed -n 3p "$S/out2.txt" | jq -c '.result.structuredContent.peers')"
[ -n "$board" ] && [ "$board" != null ] || { echo "    no board:"; cat "$S/out2.txt"; exit 1; }
row() { printf '%s' "$board" | jq -c --arg n "$1" '.[] | select(.client.name == $n) | .address'; }

a="$(row case3957-a)"
printf '%s' "$a" | jq -e --arg s "$S/a.sock" '
  .provider == "claude-code" and .session == "sess-a" and .pid == 4242
  and .socket == $s and (.host | length) > 0' >/dev/null \
  || { echo "    the server's own client is not located from its environment: $a"; exit 1; }
echo "    the server's own stdio client is located from the server's environment"

b="$(row case3957-b)"
printf '%s' "$b" | jq -e --arg s "$S/b.sock" '
  .provider == "claude-code" and .session == "sess-b" and .pid == 4343
  and .socket == $s and (.host | length) > 0' >/dev/null \
  || { echo "    the bridged client is not located from the bridge's environment: $b"; exit 1; }
echo "    a bridged client is located from what its bridge carried in initialize"

grep -qF "$TOKEN" "$S/out2.txt" && { echo "    the messaging token reached the board"; exit 1; }
grep -qF "$TOKEN" "$S/err1.txt" "$S/err2.txt" && { echo "    the messaging token reached a log"; exit 1; }
echo "    the provider's messaging token is never on the board nor in a log"

# ---------------------------------------------------------------- 3. a client with no provider
{
  req 1 initialize "$(init case3957-c)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_peers","arguments":{"checkouts":"this"}}'
} > "$S/in3"
rc=0; bare "$RB" mcp < "$S/in3" > "$S/out3.txt" 2> "$S/err3.txt" || rc=$?
[ "$rc" = 0 ] || { echo "    the third client exited $rc"; cat "$S/err3.txt"; exit 1; }
c="$(sed -n 2p "$S/out3.txt" | jq -c '.result.structuredContent.peers[] | select(.client.name == "case3957-c") | .address')"
printf '%s' "$c" | jq -e '(.host | length) > 0 and (.pid | type) == "number" and (has("provider") | not) and (has("socket") | not)' >/dev/null \
  || { echo "    a client with no provider is not named by machine and process: $c"; exit 1; }
echo "    a client that exposes no provider is still named by its machine and its process"

exec 3>&-
rc=0; wait "$p1" || rc=$?
[ "$rc" = 0 ] || { echo "    the first client's server exited $rc"; cat "$S/err1.txt"; exit 1; }

# ---------------------------------------------------------------- 4. a remote run's owner
O="$S/cluster"; H="$O/host"
mkdir -p "$O/bin" "$O/out" "$H/home/dev" "$O/seed/scripts"
G() { git -c user.name=t -c user.email=t@invalid -c init.defaultBranch=master "$@"; }
# the stand-in for ssh: options, the address, then the command, run here under the host's
# home; every command line it is handed is kept, to show what crossed
cat > "$O/bin/ssh" <<'SSH'
#!/bin/sh
while [ $# -gt 0 ]; do case $1 in -o|-i) shift 2 ;; -*) shift ;; *) break ;; esac; done
shift
printf '%s\n' "$*" >> "$CASE_CLUSTER/ssh.log"
cd "$CASE_CLUSTER/host/home" || exit 255
exec env HOME="$CASE_CLUSTER/host/home" sh -c "$*"
SSH
# the host's derive: what it was told about its owner, its own pid and group, the run record
# it runs under; held, it leaves a grandchild and waits to be stopped
cat > "$O/seed/scripts/derive" <<'DERIVE'
#!/bin/sh
out=$CASE_CLUSTER/out/$CASE_RUN
printf '%s|%s|%s|%s|%s|%s\n' "$MAJORDOMUS_OWNER_SESSION" "$MAJORDOMUS_OWNER_PEER" \
  "$MAJORDOMUS_OWNER_ISSUE" "$MAJORDOMUS_OWNER_HOST" "$MAJORDOMUS_OWNER_PID" \
  "$MAJORDOMUS_OWNER_RUN" > "$out.owner"
echo "$$ $(ps -o pgid= -p $$ | tr -d ' ')" > "$out.group"
rec="$CASE_CLUSTER/queue.runs/$MAJORDOMUS_OWNER_RUN.run"; i=0
while [ ! -f "$rec" ] && [ "$i" -lt 200 ]; do sleep 0.05; i=$((i+1)); done
cp "$rec" "$out.record"
if [ "${CASE_HOLD:-}" = 1 ]; then
  sleep 120 & echo "$!" > "$out.grandchild"
  : > "$out.started"
  wait
fi
echo "==> derive: done"
DERIVE
chmod +x "$O/bin/ssh" "$O/seed/scripts/derive"
printf '.ai/local/\n' > "$O/seed/.gitignore"
echo one > "$O/seed/a.txt"
G init -q "$O/seed" && G -C "$O/seed" add -A && G -C "$O/seed" commit -qm seed
G init -q --bare "$O/origin.git" && G -C "$O/seed" push -q "$O/origin.git" master
G clone -q "$O/origin.git" "$H/home/dev/prismatic-majordomus"
G clone -q "$O/origin.git" "$O/session"
# the session's active task names the issue
mkdir -p "$O/session/.ai/local/state"
printf 'id: t-3957\ntask: a remote run\noutcome: active\nissue: I3956\n' \
  > "$O/session/.ai/local/state/current.yaml"
cat > "$O/derive.yaml" <<YAML
schema: derive-offload/v1
kind: derive-offload
id: case3957
queue:
  directory: $O/queue
  lock: $O/lock
  poll_seconds: 1
  wait_max_seconds: 60
remote:
  spool: $O/spool
hosts:
  - id: h0
    address: u@h0
YAML
CASE_CLUSTER="$O"; export CASE_CLUSTER
# offload <session> <provider pid> <subcommand and its arguments>: as that session would
offload() {
  sid=$1; spid=$2; shift 2
  (cd "$O/session" && bare env CLAUDECODE=1 CLAUDE_CODE_SESSION_ID="$sid" CLAUDE_PID="$spid" \
    CLAUDE_CODE_MESSAGING_TOKEN="$TOKEN" MAJORDOMUS_OFFLOAD_SSH="$O/bin/ssh" \
    CASE_RUN="${CASE_RUN:-}" CASE_HOLD="${CASE_HOLD:-}" \
    "$RB" offload --declaration "$O/derive.yaml" "$@")
}

echo two > "$O/session/a.txt"
rc=0
CASE_RUN=a offload sess-r 4545 derive . --host h0 --format json \
  > "$O/a.json" 2> "$O/a.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the owned derive exited $rc"; cat "$O/a.err"; exit 1; }
tag="$(jq -r .tag "$O/a.json")"
IFS='|' read -r os op oi oh opid orun < "$O/out/a.owner"
[ "$os" = claude-code-sess-r ] && [ -z "$op" ] && [ "$oi" = I3956 ] && [ -n "$oh" ] \
  && [ "$opid" = 4545 ] && [ "$orun" = "$tag" ] \
  || { echo "    the derive was not told its owner: $(cat "$O/out/a.owner")"; exit 1; }
jq -e '.owner.session == "claude-code-sess-r" and .owner.issue == "I3956" and .owner.pid == 4545' \
  "$O/a.json" >/dev/null || { echo "    the report does not name the owner: $(jq -c .owner "$O/a.json")"; exit 1; }
echo "    the derive runs with its owner exported: session and pid from the address, issue from the task"

read -r dpid dpgid < "$O/out/a.group"
[ -n "$dpid" ] && [ "$dpid" = "$dpgid" ] \
  || { echo "    the derive does not lead a process group of its own: pid $dpid, group $dpgid"; exit 1; }
echo "    the derive leads a process group of its own"

r="$O/out/a.record"
head -1 "$r" | grep -qx 'majordomus-offload-run/v1' || { echo "    no run record:"; cat "$r"; exit 1; }
for want in "tag $tag" "pgid $dpgid" "owner_session claude-code-sess-r" "owner_issue I3956" \
  "owner_pid 4545" "owner_host $oh"; do
  grep -qx "$want" "$r" || { echo "    the run record lacks '$want':"; cat "$r"; exit 1; }
done
grep -Eq '^started_at [0-9]+$' "$r" && grep -Eq "^cwd .*/offload-$tag\$" "$r" \
  && grep -Eq '^pid [0-9]+$' "$r" && grep -Eq '^host .+' "$r" \
  || { echo "    the run record lacks its start, its cwd, its shell or its host:"; cat "$r"; exit 1; }
[ -z "$(ls "$O/queue.runs")" ] || { echo "    a run record outlived its run: $(ls "$O/queue.runs")"; exit 1; }
echo "    the run record names owner, group, tag, start and cwd while the run lives, and goes with it"

grep -qF "$TOKEN" "$O/ssh.log" "$r" "$O/a.json" "$O/a.err" \
  && { echo "    the messaging token travelled to the host"; exit 1; }
echo "    the messaging token never travels"

# ---------------------------------------------------------------- 5. a run stops as a group
G -C "$O/session" add a.txt && G -C "$O/session" commit -qm two
echo three > "$O/session/a.txt"
CASE_RUN=b CASE_HOLD=1 offload sess-r 4545 derive . --host h0 \
  > "$O/b.out" 2> "$O/b.err" & held=$!
i=0
until [ -f "$O/out/b.started" ]; do
  i=$((i+1)); [ "$i" -lt 600 ] || { echo "    the held derive never started"; cat "$O/b.err"; exit 1; }
  sleep 0.1
done
btag="$(sed -n 's/^tag //p' "$O/out/b.record")"
gc="$(cat "$O/out/b.grandchild")"
[ -f "$O/queue.runs/$btag.run" ] || { echo "    the running derive has no run record"; exit 1; }

rc=0; offload sess-x 4646 stop "$btag" --grace 3 > "$O/x.out" 2> "$O/x.err" || rc=$?
[ "$rc" = 13 ] && grep -q 'belongs to session claude-code-sess-r' "$O/x.err" \
  || { echo "    another session's stop was not refused ($rc):"; cat "$O/x.err"; exit 1; }
kill -0 "$gc" 2>/dev/null && [ -f "$O/queue.runs/$btag.run" ] \
  || { echo "    a refused stop signalled the run"; exit 1; }
echo "    another session's stop is refused, naming the owner, and nothing is signalled"

rc=0; offload sess-r 4545 stop "$btag" --grace 3 --format json > "$O/s.json" 2> "$O/s.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the owner's stop exited $rc"; cat "$O/s.err"; exit 1; }
jq -e '.signals[0] == "TERM" and .members >= 2 and .record.owner.session == "claude-code-sess-r"' \
  "$O/s.json" >/dev/null || { echo "    the stop report is not a group stop:"; cat "$O/s.json"; exit 1; }
rc=0; wait "$held" || rc=$?; held=""
[ "$rc" = 14 ] && grep -q 'stopped on h0 with `offload stop`' "$O/b.err" \
  || { echo "    the stopped derive did not say it was stopped ($rc):"; cat "$O/b.err"; exit 1; }
i=0
while kill -0 "$gc" 2>/dev/null; do
  i=$((i+1)); [ "$i" -lt 50 ] || { kill "$gc" 2>/dev/null; echo "    the grandchild outlived its group"; exit 1; }
  sleep 0.1
done
[ ! -f "$O/queue.runs/$btag.run" ] || { echo "    the stopped run left its record"; exit 1; }
echo "    the owner's stop ends the whole group, grandchild included, and the record goes with it"

# ---------------------------------------------------------------- 6. who runs this
W="$S/who"
mkdir -p "$W/bin" "$W/sessions" "$W/socks" "$W/kept" .ai/repo/offload
# the repository declares the cluster's host, as an offloading checkout does
sed 's/^id: case3957$/id: derive/' "$O/derive.yaml" > .ai/repo/offload/derive.yaml
git add .ai/repo/offload/derive.yaml
# the process table both machines answer from: pid|ppid|pgid|start|command|cwd|environment
cat > "$W/procs" <<PROCS
4957010|4957009|4957010|Thu Oct 22 15:21:51 2026|cargo test --doc|$W/kept|CLAUDE_CODE_SESSION_ID=sess-w CLAUDE_CODE_MESSAGING_TOKEN=$TOKEN
4957009|1|4957009|Thu Oct 22 15:00:00 2026|claude|$W/kept|
4957003|1|4957003|Thu Oct 22 16:00:00 2026|rustc --crate-name majordomus_cli|$W/removed-worktree|
4957020|4957019|4957019|Thu Oct 22 17:00:00 2026|rustc --edition 2021|$W/kept|MAJORDOMUS_OWNER_SESSION=claude-code-sess-r MAJORDOMUS_OWNER_ISSUE=I3956 MAJORDOMUS_OWNER_RUN=b3957f0000
4957019|1|4957019|Thu Oct 22 17:00:00 2026|sh -c derive|$W/kept|
4957030|4957031|4957030|Thu Oct 22 18:00:00 2026|make|$W/kept|
4957031|1|4957031|Thu Oct 22 18:00:00 2026|sh|$W/kept|
PROCS
# the stand-in ps: `-o pid= -o ppid= -o pgid=`, `-o lstart=`, `-o command=` (with `-E`, the
# environment after it), each for the one pid `-p` names
cat > "$W/bin/ps" <<'PS'
#!/bin/sh
fields=""; env=""; pid=""
while [ $# -gt 0 ]; do
  case $1 in -o) fields="$fields ${2%=}"; shift 2 ;; -p) pid=$2; shift 2 ;; -E) env=1; shift ;; *) shift ;; esac
done
awk -F'|' -v p="$pid" -v f="$fields" -v e="$env" '$1 == p {
  n = split(f, w, " "); out = ""
  for (i = 1; i <= n; i++) {
    v = (w[i] == "pid") ? $1 : (w[i] == "ppid") ? $2 : (w[i] == "pgid") ? $3 : (w[i] == "lstart") ? $4 : $5
    if (w[i] == "command" && e == 1 && $7 != "") v = v " " $7
    out = (out == "") ? v : out " " v
  }
  print out
}' "$CASE_WHO/procs"
PS
cat > "$W/bin/lsof" <<'LSOF'
#!/bin/sh
while [ $# -gt 0 ]; do case $1 in -p) pid=$2; shift 2 ;; *) shift ;; esac; done
awk -F'|' -v p="$pid" '$1 == p { print "p" $1; print "n" $6 }' "$CASE_WHO/procs"
LSOF
chmod +x "$W/bin/ps" "$W/bin/lsof"
# the provider's registry entry for its process, whose start matches; and a stale one, left
# by an earlier process with the leftover's pid
printf '{"pid":4957009,"sessionId":"sess-w","name":"case-3957-w","messagingSocketPath":"%s","procStart":"Thu Oct 22 15:00:00 2026"}\n' \
  "$W/socks/4957009.sock" > "$W/sessions/4957009.json"
printf '{"pid":4957003,"sessionId":"sess-stale","name":"stale","procStart":"Mon Oct 19 09:00:00 2026"}\n' \
  > "$W/sessions/4957003.json"
# the host's run records: the leftover's group, and a run on h0 the remote process carries
started="$(date +%s)"
printf 'majordomus-offload-run/v1\ntag a3957f0000\npid 4957002\npgid 4957003\nstarted_at %s\nhost h0\ncwd %s/removed-worktree\nowner_session claude-code-sess-r\nowner_peer p7\nowner_issue I3956\nowner_host case-mac\nowner_pid 4545\n' \
  "$started" "$W" > "$O/queue.runs/a3957f0000.run"
printf 'majordomus-offload-run/v1\ntag b3957f0000\npid 4957019\npgid 4957019\nstarted_at %s\nhost h0\ncwd %s/kept\nowner_session claude-code-sess-r\nowner_issue I3956\nowner_host case-mac\nowner_pid 4545\n' \
  "$started" "$W" > "$O/queue.runs/b3957f0000.run"
CASE_WHO="$W"; export CASE_WHO
# who <subject>: as a session would ask, with the stand-ins first on the path
who() {
  bare env PATH="$W/bin:$PATH" MAJORDOMUS_OFFLOAD_SSH="$O/bin/ssh" MAJORDOMUS_WHO_SOCKET_DIR="$W/socks" \
    MAJORDOMUS_WHO_SESSION_DIR="$W/sessions" "$RB" mesh who "$@"
}
: > "$O/ssh.log"
rc=0; who 4957010 --format json > "$W/w.json" 2> "$W/w.err" || rc=$?
[ "$rc" = 0 ] || { echo "    a worker's process exited $rc"; cat "$W/w.err" "$W/w.json"; exit 1; }
jq -e --arg sock "$W/socks/4957009.sock" '.known and (.leftover | not)
  and .owners[0].session == "claude-code-sess-w" and .owners[0].name == "case-3957-w"
  and .owners[0].address.pid == 4957009 and .owners[0].address.socket == $sock
  and (.chain | length) == 2' "$W/w.json" >/dev/null \
  || { echo "    a worker's process is not named by its environment and its registry entry:"; jq -c . "$W/w.json"; exit 1; }
grep -qF "$TOKEN" "$W/w.json" && { echo "    the messaging token reached the answer"; exit 1; }
echo "    a worker's process: its session from its environment, its name and socket from the registry"

rc=0; who 4957003 --format json > "$W/l.json" 2> "$W/l.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the leftover exited $rc"; cat "$W/l.err" "$W/l.json"; exit 1; }
jq -e '.known and .leftover and .owners[0].session == "claude-code-sess-r"
  and .owners[0].issue == "I3956" and .owners[0].peer == "p7" and .run.tag == "a3957f0000"
  and .owners[0].address.host == "case-mac" and .owners[0].address.pid == 4545
  and ([.owners[0].basis[] | select(test("earlier process"))] | length) == 1
  and .chain[0].cwd_gone' "$W/l.json" >/dev/null \
  || { echo "    the leftover is not named by its group's run record:"; jq -c . "$W/l.json"; exit 1; }
echo "    a leftover is marked and named by its group's run record; a stale registry entry is not used"

[ ! -s "$O/ssh.log" ] || { echo "    a local question went through ssh:"; cat "$O/ssh.log"; exit 1; }
rc=0; who h0:4957020 --format json > "$W/r.json" 2> "$W/r.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the remote process exited $rc"; cat "$W/r.err" "$W/r.json"; exit 1; }
jq -e '.known and .host == "h0" and .owners[0].session == "claude-code-sess-r"
  and .owners[0].issue == "I3956" and .run.tag == "b3957f0000"' "$W/r.json" >/dev/null \
  || { echo "    the remote process is not named:"; jq -c . "$W/r.json"; exit 1; }
[ -s "$O/ssh.log" ] || { echo "    host:pid did not go through the offload ssh path"; exit 1; }
rc=0; who h0:b3957f0000 --format json > "$W/t.json" 2> "$W/t.err" || rc=$?
[ "$rc" = 0 ] && jq -e '.known and .kind == "tag" and .owners[0].session == "claude-code-sess-r"' "$W/t.json" >/dev/null \
  || { echo "    the run tag is not named ($rc):"; cat "$W/t.err"; jq -c . "$W/t.json"; exit 1; }
echo "    host:pid and host:tag are read through the offload ssh path"

: > "$O/ssh.log"
rc=0; who nowhere:4957020 --format json > "$W/n.json" 2> "$W/n.err" || rc=$?
[ "$rc" = 10 ] && jq -e '(.known | not) and ([.missing[] | select(test("not a declared offload host"))] | length) == 1' "$W/n.json" >/dev/null \
  || { echo "    an undeclared host is not refused ($rc):"; jq -c . "$W/n.json"; exit 1; }
[ ! -s "$O/ssh.log" ] || { echo "    an undeclared host was reached:"; cat "$O/ssh.log"; exit 1; }
echo "    an undeclared host is refused, and nothing is reached"

rc=0; who I3956 --format json > "$W/i.json" 2> "$W/i.err" || rc=$?
[ "$rc" = 0 ] && jq -e '.known and .kind == "issue" and .owners[0].session == "claude-code-sess-r"
  and ([.owners[0].basis[] | select(test("run record"))] | length) == 2' "$W/i.json" >/dev/null \
  || { echo "    the issue is not answered by its run records ($rc):"; jq -c . "$W/i.json"; exit 1; }
echo "    an issue is answered by the run records that name it"

rc=0; who "$W/kept/src" --format json > "$W/p.json" 2> "$W/p.err" || rc=$?
[ "$rc" = 0 ] && jq -e '.known and .kind == "path" and (.owners | length) == 1
  and .owners[0].session == "claude-code-sess-r"' "$W/p.json" >/dev/null \
  || { echo "    a run's worktree path is not answered ($rc):"; jq -c . "$W/p.json"; exit 1; }
echo "    a worktree path is answered by the run that works in it"

for subject in 4957030 4957099; do
  rc=0; who "$subject" --format json > "$W/u.json" 2> "$W/u.err" || rc=$?
  [ "$rc" = 10 ] && jq -e '(.known | not) and (.owners | length) == 0 and (.missing | length) > 0' "$W/u.json" >/dev/null \
    || { echo "    $subject is not unknown with what is missing ($rc):"; jq -c . "$W/u.json"; exit 1; }
done
jq -e '.missing[0] | test("no such process")' "$W/u.json" >/dev/null \
  || { echo "    a pid that is not running is not said to be missing:"; jq -c . "$W/u.json"; exit 1; }
rc=0; who 4957030 > "$W/u.txt" 2>/dev/null || rc=$?
[ "$rc" = 10 ] && grep -q '^missing    session:' "$W/u.txt" && grep -q 'unknown' "$W/u.txt" \
  || { echo "    the unknown answer does not read as one ($rc):"; cat "$W/u.txt"; exit 1; }
echo "    without owner evidence the answer is unknown, exit 10, every missing fact listed"

{
  req 1 initialize "$(init case3957-who)"
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  req 2 tools/call '{"name":"majordomus_mesh_who","arguments":{"subject":"4957003"}}'
} > "$W/mcp.in"
rc=0
bare env PATH="$W/bin:$PATH" MAJORDOMUS_WHO_SOCKET_DIR="$W/socks" MAJORDOMUS_WHO_SESSION_DIR="$W/sessions" \
  "$RB" mcp < "$W/mcp.in" > "$W/mcp.out" 2> "$W/mcp.err" || rc=$?
[ "$rc" = 0 ] || { echo "    the MCP client exited $rc"; cat "$W/mcp.err"; exit 1; }
sed -n 2p "$W/mcp.out" | jq -e '.result.structuredContent | .known and .leftover
  and .owners[0].session == "claude-code-sess-r"' >/dev/null \
  || { echo "    majordomus_mesh_who over MCP does not answer the leftover:"; cat "$W/mcp.out"; exit 1; }
echo "    majordomus_mesh_who answers the same over MCP"

# ---------------------------------------------------------------- 7. a raw ssh build on a cluster host is refused
G7="$S/guard"
mkdir -p "$G7/bin"
printf '#!/bin/sh\necho "$*" >> "%s/ssh.log"\nexit 0\n' "$G7" > "$G7/bin/ssh"
chmod +x "$G7/bin/ssh"
HERE="$(pwd)"
# hook <command> [executable]: the exit code a Claude Code PreToolUse payload of the Bash tool
# running <command> in this checkout gets from `guard hook` (or from the given executable)
hook() {
  c="$1"; shift
  [ $# -gt 0 ] || set -- "$RB" guard hook --provider claude-code
  jq -nc --arg c "$c" --arg d "$HERE" \
    '{session_id:"s7",hook_event_name:"PreToolUse",tool_name:"Bash",tool_input:{command:$c},cwd:$d}' \
    | PATH="$G7/bin:$PATH" MAJORDOMUS_BIN="$RB" "$@" 2> "$G7/err"
}
for c in \
  "ssh -o BatchMode=yes h0 'cd ~/dev/p; cargo test --doc'" \
  "ssh h0 -- cargo build" \
  "ssh u@h0 env CARGO_BUILD_JOBS=3 cargo clippy" \
  "ssh -tt h0 \"nice -n 10 ~/.cargo/bin/cargo test\"" \
  "ssh h0 rustc main.rs" \
  "ssh h0 'cd dev/p && scripts/derive'" \
  "ssh h0.tailnet.ts.net ./scripts/rust-check --doc" \
  "ssh h0 \"bash -lc 'cargo test'\"" \
  "bash -c 'ssh h0 cargo test 2>&1 | tail -5'"
do
  rc=0; hook "$c" || rc=$?
  [ "$rc" = 2 ] && grep -q 'project.a-derive-offload-waits-its-turn' "$G7/err" \
    && grep -q 'majordomus offload derive --host h0' "$G7/err" && grep -q 'majordomus offload stop' "$G7/err" \
    || { echo "    a raw ssh build was not refused ($rc): $c"; cat "$G7/err"; exit 1; }
done
echo "    every raw ssh build on the declared host is refused, naming the rule and the offload"

for c in \
  "ssh h0 ps aux" \
  "ssh h0 'df -h / ; uptime'" \
  "ssh h0 'ps aux | grep cargo'" \
  "ssh u@h0 majordomus offload stop 0123456789" \
  "majordomus offload derive --host h0 ." \
  "cargo build" \
  "ssh nowhere cargo build" \
  "ssh nowhere 'cd x; scripts/derive'"
do
  rc=0; hook "$c" || rc=$?
  [ "$rc" = 0 ] && [ ! -s "$G7/err" ] || { echo "    refused ($rc): $c"; cat "$G7/err"; exit 1; }
done
echo "    a read, the offload, a local build and an undeclared host are let through"

rc=0; hook "ssh h0 cargo test" "$ROOT/.claude/hooks/majordomus-edit-guard" || rc=$?
[ "$rc" = 2 ] && grep -q 'majordomus offload derive --host h0' "$G7/err" \
  || { echo "    the tracked shim did not pass on the refusal ($rc)"; cat "$G7/err"; exit 1; }
rc=0; hook "ssh h0 uptime" "$ROOT/.claude/hooks/majordomus-edit-guard" || rc=$?
[ "$rc" = 0 ] || { echo "    the tracked shim refused a read ($rc)"; cat "$G7/err"; exit 1; }
[ ! -e "$G7/ssh.log" ] || { echo "    ssh was run:"; cat "$G7/ssh.log"; exit 1; }
echo "    the tracked shim passes the refusal on, and ssh is never run"
