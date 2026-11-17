// The results document of the audit, `ui-audit/v1`: its contract and where it is written.
//
// Apart from ui-run.mjs because ui-run.mjs drives a browser, and the readers of this shape —
// the union of the audit's shards (ui-shard.mjs) and the cases that hold it — need neither a
// browser nor playwright installed.

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

/** The contract of the results document, read back by `majordomus web report ui`. */
export const RESULTS_SCHEMA = 'ui-audit/v1';

/** Write a results document where the report renderer expects to read it. */
export function write(path, results) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(results, null, 2)}\n`);
}
