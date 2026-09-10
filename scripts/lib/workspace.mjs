// The workspace half of the transport: what a declaration says, where the store is, and
// what a record must satisfy before it is written.
//
// The declaration is read through the executable rather than parsed here. A workspace is a
// declared kind, indexed like every other, and `objects.get` already answers with it — so
// the Node layer asks the registry instead of growing a YAML parser and a second opinion
// about what the file means. The one reader is the point (ADR 0004).
//
// Nothing in this module reaches the network. It is the part that can be tested without a
// browser, which is most of the part that can be got wrong.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
export const ROOT = resolve(HERE, '..', '..');

/** The contract every synced record is held to. */
export const RECORD_SCHEMA_ID = 'majordomus.workspace-record/v1';
const RECORD_SCHEMA_PATH = 'share/schemas/majordomus/workspace-record/workspace-record.v1.schema.json';

let recordSchema = null;

/** The record schema as JSON, read once. */
export function schema() {
  if (!recordSchema) {
    const file = resolve(ROOT, RECORD_SCHEMA_PATH);
    if (!existsSync(file)) throw new Error(`the record contract is missing: ${RECORD_SCHEMA_PATH}`);
    recordSchema = JSON.parse(readFileSync(file, 'utf8'));
    if (recordSchema['x-majordomus-schema'] !== RECORD_SCHEMA_ID) {
      throw new Error(
        `${RECORD_SCHEMA_PATH} declares ${recordSchema['x-majordomus-schema']}, and this reader applies ${RECORD_SCHEMA_ID}`,
      );
    }
  }
  return recordSchema;
}

/**
 * The declaration of one workspace, as the registry reads it.
 *
 * `--quiet --format json` because this is a program reading a program: the human-facing
 * event stream on stderr is someone else's output, and a parse of it would break the first
 * time a step was renamed.
 */
export function declaration(id, { root = ROOT } = {}) {
  if (!/^[a-z][a-z0-9-]*$/.test(id)) throw new Error(`"${id}" is not a workspace id`);
  let out;
  try {
    out = execFileSync(
      resolve(root, 'bin/majordomus-cli'),
      ['run', 'objects.get', '--input', JSON.stringify({ uri: `majordomus://workspace/${id}` }), '--quiet', '--format', 'json'],
      { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 32 * 1024 * 1024 },
    );
  } catch (error) {
    const said = (error.stderr || '').trim().split('\n').slice(-3).join('\n');
    throw new Error(`no workspace "${id}" in the layer${said ? `:\n${said}` : ''}`);
  }
  const object = JSON.parse(out);
  const decl = object.metadata || object.output?.metadata;
  if (!decl || decl.kind !== 'workspace') throw new Error(`majordomus://workspace/${id} did not answer with a workspace`);
  return decl;
}

/**
 * The profile label, resolved to the key the browser configuration maps.
 *
 * Namespaced by vendor because the labels are the operator's own words: two workspaces at
 * two vendors will both be called `default` on the day a second one is declared, and a
 * collision there would sign a transport into the wrong account.
 */
export function profileKey(decl) {
  const label = decl.access?.browser_profile;
  if (!label) throw new Error(`workspace ${decl.id} declares no access.browser_profile`);
  return `${decl.vendor.id}:${label}`;
}

/** The origins an adapter for this workspace may observe, and no others. */
export function origins(decl) {
  const list = decl.vendor?.origins || [];
  if (!list.length) throw new Error(`workspace ${decl.id} declares no vendor.origins; there is nothing it is allowed to observe`);
  return list;
}

/** Where this workspace's content lives: this checkout's state, never the tracked tree. */
export function storeDir(id, { root = ROOT } = {}) {
  return resolve(root, '.ai/local/workspaces', id);
}

/** A stable hash over a normalised body: the same content twice is the same 64 hex digits. */
export function contentHash(body) {
  return createHash('sha256').update(stable(body === undefined ? null : body)).digest('hex');
}

/** JSON with its object keys in a fixed order, so a hash is a fact about content and not about insertion. */
function stable(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value) ?? 'null';
  if (Array.isArray(value)) return `[${value.map(stable).join(',')}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stable(value[k])}`).join(',')}}`;
}

/**
 * Build a record, and refuse to build one that the contract would not accept.
 *
 * The refusal matters more than the construction. The failure this milestone must prevent
 * is not the loud one; it is a parse that half succeeds and writes plausible nonsense into
 * the store, where it is indistinguishable from something observed.
 */
export function makeRecord({
  workspace,
  vendor,
  type,
  upstreamId,
  transport,
  acquisition,
  support,
  contract,
  parents = [],
  title,
  body,
  firstSeen,
  lastSeen,
}) {
  const now = new Date().toISOString();
  const record = {
    schema: 'workspace-record/v1',
    workspace,
    vendor,
    id: `${vendor}:${type}:${upstreamId}`,
    upstream_id: upstreamId,
    type,
    transport,
    acquisition,
    support,
    content_hash: contentHash(body),
    first_seen: firstSeen || now,
    last_seen: lastSeen || now,
  };
  if (contract) record.contract = contract;
  if (parents.length) record.parents = parents;
  if (title !== undefined) record.title = title;
  if (body !== undefined) record.body = body;
  const problems = validate(record);
  if (problems.length) throw new Error(`the record for ${upstreamId} does not satisfy ${RECORD_SCHEMA_ID}:\n  ${problems.join('\n  ')}`);
  return record;
}

/**
 * Check a record against the contract, reading the contract rather than restating it.
 *
 * Not a general JSON Schema implementation, and it does not pretend to be one: it applies
 * the parts this repository's schemas actually use — required, additionalProperties, enum,
 * const, pattern, type — from the file, so that editing the schema changes the check.
 * A validator that hardcoded the field list would agree with an outdated schema forever.
 */
export function validate(record, node = schema(), path = '') {
  const problems = [];
  const at = path || 'the record';

  if (node.const !== undefined && record !== node.const) {
    problems.push(`${at} must be ${JSON.stringify(node.const)}, and is ${JSON.stringify(record)}`);
    return problems;
  }
  if (node.enum && !node.enum.includes(record)) {
    problems.push(`${at} must be one of ${node.enum.join(', ')}, and is ${JSON.stringify(record)}`);
    return problems;
  }
  if (node.type === 'string') {
    if (typeof record !== 'string') return [`${at} must be a string`];
    if (node.pattern && !new RegExp(node.pattern).test(record)) {
      problems.push(`${at} does not match ${node.pattern}: ${JSON.stringify(record)}`);
    }
    if (node.minLength !== undefined && record.length < node.minLength) {
      problems.push(`${at} is shorter than ${node.minLength}`);
    }
    return problems;
  }
  if (node.type === 'array') {
    if (!Array.isArray(record)) return [`${at} must be an array`];
    if (node.items) record.forEach((item, i) => problems.push(...validate(item, node.items, `${at}[${i}]`)));
    return problems;
  }
  if (node.type === 'object' || node.properties) {
    if (record === null || typeof record !== 'object' || Array.isArray(record)) return [`${at} must be an object`];
    for (const key of node.required || []) {
      if (record[key] === undefined) problems.push(`${at} is missing ${key}`);
    }
    if (node.additionalProperties === false && node.properties) {
      for (const key of Object.keys(record)) {
        if (!(key in node.properties)) problems.push(`${at} carries ${key}, which the contract does not allow`);
      }
    }
    for (const [key, sub] of Object.entries(node.properties || {})) {
      if (record[key] !== undefined) problems.push(...validate(record[key], sub, path ? `${path}.${key}` : key));
    }
    return problems;
  }
  return problems;
}
