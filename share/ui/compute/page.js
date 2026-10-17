// The page half of /ui/compute (ADR 0170, I2788). Nothing computes until the person submits
// the form. Then the Worker the page names (`data-compute-worker`) runs the committed wasm32
// module (`data-compute-module`) on the JSON in the form, and the digest it answers is sent
// to `compute.verify` (`data-compute-verify`) as a claim: the verdict shown is the server's.
//
// Asked to, the page claims a digest with one digit changed, so that a reader sees the server
// refuse a wrong claim and not only accept a right one. A copy of the page with no server
// behind it (the published snapshot) still computes, and says that nobody recomputed it.

const holder = document.querySelector('[data-compute]');
if (holder) wire(holder);

function wire(root) {
  const form = root.querySelector('[data-compute-form]');
  const needsScript = root.querySelector('[data-compute-needs-script]');
  if (!form) return;
  const out = (key, text) => {
    const field = root.querySelector(`[data-compute-out="${key}"]`);
    if (field) field.textContent = text;
  };
  // relative to the page, never to the Worker, whose own URL is elsewhere
  const moduleUrl = new URL(root.dataset.computeModule, document.baseURI).href;
  const verifyUrl = root.dataset.computeVerify;
  let worker = null;
  let asked = 0;

  form.hidden = false;
  if (needsScript) needsScript.hidden = true;

  function compute(input) {
    if (!worker) worker = new Worker(root.dataset.computeWorker);
    return new Promise((resolve) => {
      worker.onmessage = (event) => resolve(event.data || { ok: false, error: 'no answer' });
      worker.onerror = (event) => resolve({ ok: false, error: event.message || 'the Worker failed' });
      worker.postMessage({ module: moduleUrl, input });
    });
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
