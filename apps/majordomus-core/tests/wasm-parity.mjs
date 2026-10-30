// The wasm32 module, executed: every row of the shared fixture through the C ABI a browser
// Worker calls (mj_alloc, mj_canonical_digest, mj_free), compared with the digest the
// fixture records. Node runs the same WebAssembly a browser loads; nothing here computes a
// digest of its own.
//
//   node apps/majordomus-core/tests/wasm-parity.mjs <module.wasm> <fixture.json>
//
// Exit 0 when every row agrees and every invalid input is refused, 1 otherwise, 2 when the
// module or the fixture cannot be read.

import { readFileSync } from 'node:fs';

const [modulePath, fixturePath] = process.argv.slice(2);
if (!modulePath || !fixturePath) {
  console.error('usage: wasm-parity.mjs <module.wasm> <fixture.json>');
  process.exit(2);
}

let bytes;
let fixture;
try {
  bytes = readFileSync(modulePath);
  fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
} catch (error) {
  console.error(`wasm-parity: ${error.message}`);
  process.exit(2);
}

const { instance } = await WebAssembly.instantiate(bytes, {});
const { memory, mj_alloc: alloc, mj_free: free, mj_canonical_digest: digest } = instance.exports;

function call(text) {
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

let failed = 0;
for (const c of fixture.cases) {
  const { status, payload } = call(c.input);
  const ok = status === 0 && payload === c.digest;
  if (!ok) failed += 1;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${c.id}${ok ? '' : ` status=${status} got=${payload} want=${c.digest}`}`);
}
for (const c of fixture.invalid) {
  const { status } = call(c.input);
  const ok = status === 1;
  if (!ok) failed += 1;
  console.log(`${ok ? 'ok  ' : 'FAIL'} invalid ${c.id}${ok ? '' : ' was digested'}`);
}
const total = fixture.cases.length + fixture.invalid.length;
console.log(`wasm-parity: ${total - failed} of ${total} rows agree`);
process.exit(failed === 0 ? 0 : 1);
