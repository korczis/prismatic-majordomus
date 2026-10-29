# majordomus-covers: none
# majordomus-skip-runs-in: ui
# One click implements an issue, and the execution it starts outlives the page and the server
# (I3985, ADR 0225: the owner's P0 Definition of Done of 2026-10-23, as a test). A real browser
# over the real UI, a real server killed and started again, and a fixture worker standing in
# for Claude Code, never a provider:
#
#   offered     /ui/idd offers one primary action for the ready issue, Implement I0001, and
#               the issue's move panel carries the Implement button
#   once        the button is double-clicked: one execution is started, not two
#   live        the execution's own page shows it running at the worker's step, its steps
#               table following the execution as it moves; the page is closed
#   serial      a second Implement while one runs is queued behind it, never run beside it
#   killed      the server is killed (SIGKILL) while the worker is still working, and started
#               again: the same execution id is there, failed with code `interrupted` at the
#               sequence it had reached, and neither it nor the queued one is run again
#   refused     an issue that is not ready is refused naming what it waits for
#   no worker   with no worker adapter on this machine, Implement is refused naming the
#               adapter and why it is unavailable
#   done        with the worker back, Implement runs to the end: the worker writes the work
#               into the issue's own worktree and the issue's validation passes there
#   restarted   the server is restarted once more: the finished execution is still there,
#               succeeded, with the validation it ran and its result
#   clean       no uncaught exception and no failed request on any page
#
# Not walked here: a scope another live claim holds. The fixture's server runs no mesh, so its
# lease step says nothing else can hold the scope. That refusal (claim_conflict) has no test
# yet: it needs a process whose mesh runs, and is left open on I3985.
#
# Needs a browser scripts/lib/browser.mjs can launch, so it runs in the ui job
# (majordomus-skip-runs-in: ui). A local run without CI walks it too.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
if [ -n "${CI:-}" ] && [ "${MAJORDOMUS_RUNS_IN:-}" != ui ]; then
  skip "one-click implement is walked in the ui job (--runs-in ui), not in this CI run"
fi
command -v node >/dev/null 2>&1 || skip "node is absent, so one-click implement was not walked"
[ -d "$ROOT/node_modules/playwright" ] \
  || skip "playwright is not installed (npm ci), so one-click implement was not walked"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
pj_init
pj_milestone loop 1
pj_issue I0001 loop
pj_issue I0002 loop I0001
sed -i.bak 's#^title: Issue I0001$#title: The answer is written#' .ai/repo/project/issues/I0001.yaml
sed -i.bak 's#^title: Issue I0002$#title: The answer is read#' .ai/repo/project/issues/I0002.yaml
mkdir -p checks .ai/repo/project/intents
cat > checks/the-answer-holds.sh <<'SH'
#!/bin/sh
# The check I0001's work answers to: the answer it owes is in its scope.
if [ "$(cat src/I0001/answer.txt 2>/dev/null)" = 42 ]; then echo "the answer holds"; else echo "the answer does not hold"; exit 1; fi
SH
sed -i.bak 's#^  - "true"$#  - "sh checks/the-answer-holds.sh"#' .ai/repo/project/issues/I0001.yaml
rm -f .ai/repo/project/issues/*.bak
printf 'serves:\n  - the-answer-is-known#the-answer-holds\n' >> .ai/repo/project/issues/I0001.yaml
cat > .ai/repo/project/intents/the-answer-is-known.yaml <<'YAML'
id: the-answer-is-known
title: The answer is known
statement: "Whoever asks finds the answer where the work put it."
invariants:
  - The answer is written once, in the scope of the issue that owes it
milestones:
  - loop
satisfaction:
  - id: the-answer-holds
    criterion: The answer the work owes is in its scope
    evidence: test
    ref: checks/the-answer-holds.sh
YAML
git add -A >/dev/null && git commit -qm "an intent, an issue that serves it and one that waits for it"

# The fixture worker answers to the name the Claude Code adapter looks for on PATH. It works in
# the directory it is started in, the issue's worktree, and waits for the gate the case opens,
# so that the server can be killed while it is still working.
FIXTURE="$PWD/.fixture"; export FIXTURE
mkdir -p "$FIXTURE/worker" "$FIXTURE/none"
cat > "$FIXTURE/worker/claude" <<'SH'
#!/bin/sh
echo "$$" > "$FIXTURE/worker.pid"
pwd > "$FIXTURE/worker.cwd"
i=0
while [ ! -f "$FIXTURE/gate" ]; do
  i=$((i+1)); [ "$i" -lt 1200 ] || { echo "the gate never opened" >&2; exit 3; }
  sleep 0.1
done
mkdir -p src/I0001 && echo 42 > src/I0001/answer.txt
echo '{"type":"result","subtype":"success","result":"the answer is written"}'
SH
chmod +x "$FIXTURE/worker/claude"
# PATH without any `claude`: every directory of it that holds one is left out
NO_WORKER_PATH=""
old_ifs="$IFS"; IFS=:
for d in $PATH; do
  [ -n "$d" ] && [ ! -e "$d/claude" ] && NO_WORKER_PATH="${NO_WORKER_PATH:+$NO_WORKER_PATH:}$d"
done
IFS="$old_ifs"
WITH_WORKER_PATH="$FIXTURE/worker:$NO_WORKER_PATH"
REPO="$PWD"

SRV=""
cleanup() {
  serve_down
  # a worker that already ended, or a worktree already gone, is not a failure of the case
  if [ -f "$FIXTURE/worker.pid" ]; then kill "$(cat "$FIXTURE/worker.pid")" 2>/dev/null || true; fi
  # the issue's worktree is the fixture's sibling, as every worktree of a checkout is
  rm -rf "$REPO-wt"
  git -C "$REPO" worktree prune >/dev/null 2>&1 || true
}
trap cleanup EXIT
# up PATH — start the server with that PATH, so its workers find the adapters on it
up() { PATH="$1" serve_up "serve.$2.out" "serve.$2.err"; }

cat > walk.mjs <<'JS'
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
const [phase, base, browserLib, shots, fixture] = process.argv.slice(2);
const { launch } = await import(browserLib);
const browser = await launch();
if (!browser) { console.log('NO-BROWSER'); process.exit(3); }
const WAIT = 60000;
const problems = [];
let step = phase;
const fail = (why) => { console.log('    ' + why); process.exitCode = 1; throw new Error(why); };
const watch = (page, name) => {
  page.on('pageerror', (e) => problems.push(`${name}: uncaught ${e.message}`));
  page.on('response', (r) => {
    if (r.status() >= 400) problems.push(`${name}: ${r.status()} ${new URL(r.url()).pathname}`);
  });
  page.on('console', (m) => {
    if (m.type() === 'error' && !m.text().startsWith('Failed to load resource')) problems.push(`${name}: console ${m.text()}`);
  });
  page.setDefaultTimeout(WAIT);
  page.on('dialog', (d) => d.accept()); // Implement asks to confirm a worker; the walker confirms
};
const api = async (path, init) => {
  const r = await fetch(base + path, init);
  return { status: r.status, body: await r.json().catch(() => null) };
};
const executions = async () => {
  const r = await api('/api/v1/executions');
  const list = Array.isArray(r.body) ? r.body : (r.body && (r.body.executions || r.body.items)) || [];
  return list.filter((e) => (e.execution || e).capability === 'implement.issue').map((e) => e.execution || e);
};
const one = async (id) => (await api('/api/v1/executions/get?id=' + encodeURIComponent(id))).body;
const ended = async (id, what) => {
  for (let i = 0; i < 600; i++) {
    const e = await one(id);
    const state = e && (e.execution || e).state;
    if (['succeeded', 'failed', 'cancelled'].includes(state)) return e.execution || e;
    await new Promise((r) => setTimeout(r, 100));
  }
  fail(`${what}: ${id} never ended`);
};
// start implement.issue over the API the button posts to; answers the execution's id
const startIssue = async (issue) => {
  const r = await api('/api/v1/executions/start', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ capability: 'implement.issue', input: { issue } }),
  });
  if (!r.body || !r.body.id) fail(`starting ${issue} answered ${r.status} ${JSON.stringify(r.body)}`);
  return r.body.id;
};
const state = JSON.parse(existsSync(`${fixture}/walk.json`) ? readFileSync(`${fixture}/walk.json`, 'utf8') : '{}');
const save = () => writeFileSync(`${fixture}/walk.json`, JSON.stringify(state));
// click Implement on I0001's move panel, reached from the workspace's primary action
const implement = async (page, clicks) => {
  await page.goto(`${base}/ui/idd?intent=the-answer-is-known`, { waitUntil: 'networkidle' });
  const primary = page.locator('a.mj-btn--primary');
  if (await primary.count() !== 1) fail(`the workspace offers ${await primary.count()} primary actions, not one`);
  const asks = (await primary.innerText()).trim();
  if (asks !== 'Implement I0001') fail(`the primary action reads "${asks}", not "Implement I0001"`);
  await primary.click();
  const button = page.locator('#move button[data-mj-implement]');
  await page.waitForSelector('#move button[data-mj-implement]:not([disabled])');
  if (clicks === 2) await button.dblclick(); else await button.click();
  const result = page.locator('#move [data-mj-result]');
  await page.waitForFunction(() => {
    const t = document.querySelector('#move [data-mj-result]')?.innerText || '';
    return t && !t.startsWith('starting');
  });
  return (await result.innerText()).trim();
};

try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1200 } });
  watch(page, phase);

  if (phase === 'first') {
    step = 'once';
    const said = await implement(page, 2);
    const m = said.match(/execution (\S+) is still running/);
    if (!m) fail(`Implement answered "${said}", not a running execution`);
    state.first = m[1];
    const all = await executions();
    if (all.length !== 1) fail(`a double click started ${all.length} executions, not one`);
    console.log(`    once: a double click on Implement started one execution, ${state.first}`);

    step = 'live';
    await page.click('#move [data-mj-result] a');
    await page.waitForURL(/\/cockpit\/executions\//);
    await page.waitForFunction(() => /running/i.test(document.querySelector('main')?.innerText || '')
      && /Hand the work to a worker/.test(document.querySelector('main')?.innerText || ''))
      .catch(async () => {
        const x = await one(state.first);
        const shown = (await page.locator('main').innerText()).replace(/\s+/g, ' '); writeFileSync(fixture + '/live.txt', shown);
        fail(`the execution page never showed the execution running at its worker (${x && x.state}): ${shown.slice(shown.indexOf('STEPS'), shown.indexOf('STEPS') + 700)}`);
      });
    // the step is entered before the worker's process writes its first line
    for (let i = 0; i < 100 && !existsSync(`${fixture}/worker.pid`); i++) await new Promise((r) => setTimeout(r, 100));
    if (!existsSync(`${fixture}/worker.pid`)) fail('the execution reads running but no worker was started');
    await page.screenshot({ path: `${shots}/1-running.png`, fullPage: true });
    console.log('    live: the execution page shows it running at the worker\'s step; the page is closed');

    step = 'serial';
    // implementing is serial: a second start while one runs waits for it, and says so
    const q = await startIssue('I0002');
    await new Promise((r) => setTimeout(r, 1000));
    const waiting = await one(q);
    if ((waiting.execution || waiting).state !== 'queued') {
      fail(`a second Implement while one runs reads ${(waiting.execution || waiting).state}, not queued`);
    }
    state.queued = q;
    console.log(`    serial: a second Implement while ${state.first} runs is queued behind it`);
  }

  if (phase === 'second') {
    step = 'killed';
    await page.goto(`${base}/cockpit/executions/${encodeURIComponent(state.first)}`, { waitUntil: 'networkidle' });
    const e = await one(state.first);
    const x = e && (e.execution || e);
    if (!x) fail(`after the restart ${state.first} is gone`);
    if (x.state !== 'failed' || !x.error || x.error.code !== 'interrupted') {
      fail(`after the restart ${state.first} reads ${x.state} (${x.error && x.error.code}), not failed interrupted`);
    }
    const text = await page.locator('main').innerText();
    if (!text.includes('interrupted')) fail('the execution page does not say the execution was interrupted');
    await page.screenshot({ path: `${shots}/2-interrupted.png`, fullPage: true });
    console.log(`    killed: after the restart ${state.first} is there, failed interrupted at sequence ${x.last_sequence}`);

    step = 'refused';
    // what was queued when the server was killed is not run behind anyone's back either
    const queued = await one(state.queued);
    if ((queued.execution || queued).state !== 'failed') {
      fail(`after the restart the queued ${state.queued} reads ${(queued.execution || queued).state}`);
    }
    const e2 = await ended(await startIssue('I0002'), 'refused');
    const why2 = e2.error ? `${e2.error.code}: ${e2.error.message}` : '';
    if (e2.state !== 'failed' || !/not_ready: I0002/.test(why2) || !/I0001/.test(why2)) {
      fail(`a not-ready issue ended ${e2.state} (${why2}), not refused naming what it waits for`);
    }
    console.log(`    refused: I0002 is refused — ${why2}`);

    step = 'no worker';
    const said = await implement(page, 1);
    const m = said.match(/execution (\S+) is still running/);
    if (!m) fail(`Implement answered "${said}"`);
    const n = await ended(m[1], 'no worker');
    const why = n.error ? `${n.error.code}: ${n.error.message}` : '';
    if (n.state !== 'failed' || !/no_worker: claude-code: /.test(why)) {
      fail(`with no worker the execution ended ${n.state} (${why}), not refused naming the adapter`);
    }
    console.log(`    no worker: refused — ${why}`);
  }

  if (phase === 'third') {
    step = 'done';
    writeFileSync(`${fixture}/gate`, 'open\n');
    const said = await implement(page, 1);
    const m = said.match(/execution (\S+) is still running/);
    if (!m) fail(`Implement answered "${said}"`);
    state.done = m[1];
    const e = await ended(state.done, 'done');
    if (e.state !== 'succeeded') fail(`the execution ended ${e.state}: ${JSON.stringify(e.error)}`);
    const out = e.output || {};
    const v = (out.verified || [])[0];
    if (out.state !== 'done' || !v || v.command !== 'sh checks/the-answer-holds.sh' || !v.ok) {
      fail(`the execution succeeded without its verification: ${JSON.stringify(out)}`);
    }
    state.worktree = out.worktree;
    console.log(`    done: ${state.done} succeeded; \`${v.command}\` passed in ${out.worktree}`);
  }

  if (phase === 'fourth') {
    step = 'restarted';
    await page.goto(`${base}/cockpit/executions/${encodeURIComponent(state.done)}`, { waitUntil: 'networkidle' });
    const e = await one(state.done);
    const x = e && (e.execution || e);
    if (!x || x.state !== 'succeeded') fail(`after the restart ${state.done} reads ${x && x.state}`);
    const v = ((x.output || {}).verified || [])[0];
    if (!v || !v.ok) fail('after the restart the execution no longer carries its verification');
    const first = await one(state.first);
    if ((first.execution || first).state !== 'failed') fail(`${state.first} was run again after a restart`);
    const text = await page.locator('main').innerText();
    if (!/succeeded/i.test(text)) fail('the execution page does not say the execution succeeded');
    await page.screenshot({ path: `${shots}/3-done.png`, fullPage: true });
    console.log(`    restarted: ${state.done} is still succeeded with its verification; ${state.first} was never run again`);
  }

  if (problems.length) fail('the pages reported: ' + problems.join(' | '));
} catch (error) {
  console.log(`    the walk stopped at "${step}": ${error.message}`);
  process.exitCode = 1;
} finally {
  save();
  await browser.close();
}
JS
mkdir -p shots
walk() {
  local rc=0
  node walk.mjs "$1" "$U" "$ROOT/scripts/lib/browser.mjs" "$PWD/shots" "$FIXTURE" > "walk.$1.out" 2>&1 || rc=$?
  cat "walk.$1.out"
  case "$rc" in
    0) ;;
    3) skip "no browser could be launched, so one-click implement was not walked" ;;
    *) echo "    the walk failed at its $1 phase (exit $rc)"; for f in serve.*.err; do echo "    -- $f"; tail -n 20 "$f" | sed 's/^/    /'; done; exit 1 ;;
  esac
}

# ---------------------------------------------------------------- the walk
up "$WITH_WORKER_PATH" 1 || exit 1
walk first
# killed, not stopped: nothing the server would do on its way out is done
kill -9 "$SRV" 2>/dev/null; wait "$SRV" 2>/dev/null || true; SRV=""
if [ -f "$FIXTURE/worker.pid" ]; then kill "$(cat "$FIXTURE/worker.pid")" 2>/dev/null || true; fi
rm -f "$FIXTURE/worker.pid"

up "$NO_WORKER_PATH" 2 || exit 1
walk second
serve_down

up "$WITH_WORKER_PATH" 3 || exit 1
walk third
serve_down

up "$WITH_WORKER_PATH" 4 || exit 1
walk fourth
serve_down

# the work is where the issue's worktree is, and the checkout the case started in is untouched
wt="$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).worktree||"")' "$FIXTURE/walk.json")"
[ "$(cat "$wt/src/I0001/answer.txt" 2>/dev/null)" = 42 ] || { echo "    the work is not in the issue's worktree ($wt)"; exit 1; }
[ ! -e src/I0001/answer.txt ] || { echo "    the worker wrote into the checkout, not the issue's worktree"; exit 1; }
echo "    one click implemented I0001 across a killed server, and the refusals held"
