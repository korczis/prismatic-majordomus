// The Worker of /ui/compute (ADR 0170, I2788): it runs the portable core's wasm32 module
// through the C ABI the module exports (mj_alloc, mj_canonical_digest, mj_free) and answers
// the page with what the module computed. It computes nothing of its own: the digest is the
// module's, and the page asks the server to recompute it before it says anything is right.
//
// This script's response alone carries a policy that allows WebAssembly compilation
// ('wasm-unsafe-eval'); every page keeps the Cockpit's, which does not. The module is
// fetched from this origin, the only one the policy names.
//
// A message is { module: <URL of the .wasm>, input: <JSON text> }; the answer is
// { ok: true, digest, millis } or { ok: false, error }.

let instance = null;
let loadedFrom = null;

async function load(url) {
  if (instance && loadedFrom === url) return instance;
  const response = await fetch(url, { credentials: 'same-origin' });
  if (!response.ok) throw new Error(`the module answered ${response.status}`);
  const { instance: fresh } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
  instance = fresh;
  loadedFrom = url;
  return instance;
}

// One call through the C ABI: the input copied into the module's memory, the answer read
// back as status (one byte), length (four bytes, little-endian) and payload, then both
// buffers handed back to the module's allocator.
function canonicalDigest(module, text) {
  const { memory, mj_alloc: alloc, mj_free: free, mj_canonical_digest: digest } = module.exports;
  const input = new TextEncoder().encode(text);
  const ptr = alloc(input.length);
  new Uint8Array(memory.buffer, ptr, input.length).set(input);
  const out = digest(ptr, input.length);
  const view = new DataView(memory.buffer);
  const status = view.getUint8(out);
  const len = view.getUint32(out + 1, true);
  const payload = new TextDecoder().decode(new Uint8Array(memory.buffer, out + 5, len));
  free(ptr, input.length);
  free(out, 5 + len);
  return { status, payload };
}

self.onmessage = async (event) => {
  const { module, input } = event.data || {};
  try {
    const loaded = await load(module);
    const started = performance.now();
    const { status, payload } = canonicalDigest(loaded, String(input));
    const millis = Math.round((performance.now() - started) * 1000) / 1000;
    self.postMessage(status === 0
      ? { ok: true, digest: payload, millis }
      : { ok: false, error: payload });
  } catch (error) {
    self.postMessage({ ok: false, error: String(error && error.message || error) });
  }
};
