# PHASE 07 — FUTURE EDITOR ADAPTER PLAYBOOK (DESIGN/TEST ONLY)

This is documentation and conformance testing, NOT authorization to implement unrequested editors. Validate that a developer can add a new editor without changing canonical repo discovery, policy resolution, provenance, workflow IDs or quality-gate semantics.

Required contract: adapter ID and versions; supported project files; supported capability map and unsupported reasons; mapping from semantic policy to editor-specific syntax; read/inspect/diff/safe sync; override ownership; formatter/LSP/task/schema semantics; per-editor path and escaping rules; deterministic render; explainability; conformance fixture; migration strategy; stable structured outputs; security rules; documentation.

Compare future cases: VS Code (workspace settings/tasks/extensions recommendations), JetBrains (IDE project files/inspection profiles/run configurations, machine-specific risk), Neovim (Lua config and plugin ecosystem fragmentation), Helix (TOML language server/formatters). These are examples, not assertions of exact feature compatibility. Never promise universal features. Implement one mock adapter to prove extension seam is real and ensure no Zed-specific dependency in domain core.

Done when a hypothetical unsupported editor can be registered with an explicit capability matrix and report `unsupported` accurately, without producing misleading files or requiring changes to canonical data types. Avoid maintaining a giant editor feature catalog that will become stale.
