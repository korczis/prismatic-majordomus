// The page half of /ui/compute (ADR 0170, I2788, I2809). Nothing computes until the person
// asks. Then the Worker the page names (`data-compute-worker`) runs the committed wasm32
// module (`data-compute-module`), and every digest it answers is sent to `compute.verify`
// (`data-compute-verify`) as a claim: the verdict shown is the server's.
//
// Two things ask: the form, for the JSON the reader writes (asked to, it claims a digest with
// one digit changed, so that the server is seen to refuse a wrong claim too), and the shared
// fixture's table (`data-compute-fixture`), whose every row is computed here and judged by the
// server, beside what the fixture records. A copy of the page with no server behind it (the
// published snapshot) still computes, and says that nobody recomputed it.

const holder = document.querySelector('[data-compute]');
if (holder) {
  const engine = engineOf(holder);
  wireForm(holder, engine);
  const fixture = document.querySelector('[data-compute-fixture]');
  if (fixture) wireFixture(fixture, engine);
}

// One Worker for the page, asked one input at a time.
function engineOf(root) {
  // relative to the page, never to the Worker, whose own URL is elsewhere
  const moduleUrl = new URL(root.dataset.computeModule, document.baseURI).href;
  const verifyUrl = root.dataset.computeVerify;
  let worker = null;
  let queue = Promise.resolve();

  function compute(input) {
    if (!worker) worker = new Worker(root.dataset.computeWorker);
    const turn = queue.then(() => new Promise((resolve) => {
      worker.onmessage = (event) => resolve(event.data || { ok: false, error: 'no answer' });
      worker.onerror = (event) => resolve({ ok: false, error: event.message || 'the Worker failed' });
      worker.postMessage({ module: moduleUrl, input });
    }));
    queue = turn.catch(() => {});
    return turn;
  }

  async function verify(input, claimed) {
    const response = await fetch(verifyUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
      body: JSON.stringify({ function: 'canonical_digest', input, claimed }),
    });
    if (!response.ok) throw new Error(`the server answered ${response.status}`);
    const answer = await response.json();
    return answer.output || answer;
  }

  return { compute, verify };
}

function wireForm(root, { compute, verify }) {
  const form = root.querySelector('[data-compute-form]');
  const needsScript = root.querySelector('[data-compute-needs-script]');
  if (!form) return;
  const out = (key, text) => {
    const field = root.querySelector(`[data-compute-out="${key}"]`);
    if (field) field.textContent = text;
  };
  let asked = 0;

  form.hidden = false;
  if (needsScript) needsScript.hidden = true;

  // one changed hex digit: the claim is still the shape of a digest, and still wrong
  function tampered(digest) {
    const last = digest.slice(-1);
    return digest.slice(0, -1) + (last === '0' ? '1' : '0');
  }

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    const turn = ++asked;
    const input = form.elements.input.value;
    const tamper = form.elements.tamper && form.elements.tamper.checked;
    root.dataset.computeState = 'computing';
    for (const key of ['browser', 'millis', 'verdict', 'recomputed', 'reason']) out(key, '…');
    const result = await compute(input);
    if (turn !== asked) return;
    if (!result.ok) {
      out('browser', '—');
      out('millis', '—');
      out('reason', `the module refused the input: ${result.error}`);
    } else {
      out('browser', result.digest);
      out('millis', String(result.millis));
    }
    const claimed = result.ok ? (tamper ? tampered(result.digest) : result.digest) : '';
    try {
      const verdict = await verify(input, claimed);
      if (turn !== asked) return;
      out('verdict', verdict.verdict || '—');
      out('recomputed', verdict.recomputed || '—');
      if (verdict.reason) out('reason', verdict.reason);
      else if (result.ok) out('reason', tamper ? 'the claim was changed on purpose' : '—');
      root.dataset.computeState = verdict.verdict || 'answered';
    } catch (error) {
      if (turn !== asked) return;
      out('verdict', 'not verified');
      out('recomputed', '—');
      out('reason', `no server recomputed it here (${error.message}); a browser's answer alone is only a claim`);
      root.dataset.computeState = 'unverified';
    }
  });
}

// Every row of the shared fixture: a case must be computed to the digest the fixture records
// and answered `match`; an invalid row must be refused by the module and answered `invalid`.
function wireFixture(table, { compute, verify }) {
  const run = table.querySelector('[data-fixture-run]');
  const summary = table.querySelector('[data-fixture-summary]');
  if (!run) return;
  run.disabled = false;
  run.addEventListener('click', async () => {
    run.disabled = true;
    table.dataset.fixtureState = 'running';
    const rows = Array.from(table.querySelectorAll('tr[data-fixture-row]'));
    let held = 0;
    let unverified = 0;
    const broken = [];
    for (const row of rows) {
      const cell = (key, text) => {
        const field = row.querySelector(`[data-fixture-out="${key}"]`);
        if (field) field.textContent = text;
      };
      const kind = row.dataset.fixtureKind;
      const input = row.dataset.fixtureInput;
      const expected = row.dataset.fixtureDigest;
      cell('browser', '…');
      const result = await compute(input);
      cell('browser', result.ok ? result.digest : `refused: ${result.error}`);
      const agrees = kind === 'case' ? result.ok && result.digest === expected : !result.ok;
      cell('agrees', agrees ? 'yes' : 'no');
      let verdict = 'not verified';
      try {
        verdict = (await verify(input, result.ok ? result.digest : '')).verdict || '—';
      } catch (error) {
        unverified += 1;
      }
      cell('verdict', verdict);
      const wanted = kind === 'case' ? 'match' : 'invalid';
      const holds = agrees && verdict === wanted;
      row.dataset.fixtureHolds = holds ? 'yes' : 'no';
      if (holds) held += 1;
      else broken.push(row.dataset.fixtureRow);
    }
    const line = `${held} of ${rows.length} rows hold in this browser`
      + (unverified ? `; ${unverified} not verified, no server answered` : '')
      + (broken.length ? `; not holding: ${broken.join(', ')}` : '');
    if (summary) summary.textContent = ` ${line}.`;
    table.dataset.fixtureState = broken.length ? 'broken' : 'held';
    table.dataset.fixtureHeld = String(held);
    run.disabled = false;
  });
}
