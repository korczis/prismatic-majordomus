<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `shell` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.7.0 -->
# Module `shell` — Shell automation

The repository's shell automation measured against the tracked migration inventory: new shell is refused unless an exemption declares why it has to be shell and when it stops being, and an exemption whose file is gone is refused too, so the list only shrinks as units move into typed and scripted capabilities (ADR 0069).

Stability: behaviorally_verified. Capabilities: 1.

## `shell.check` — Refuse undeclared shell

Every shell unit under bin/, lib/, scripts/, share/, .githooks/ and .claude/hooks/ — by interpreter line or by a .sh name, as git would commit it, links not followed — against .ai/repo/automation/inventory.jsonl: an undeclared unit, a record whose unit is gone, an exemption missing its reason or its removal condition, a disposition outside A-F, a duplicate and a record out of canonical order are each a finding that names the file and the remedy. Reads the tree and git's index only; no build and no network.

| | |
|---|---|
| kind | query |
| stability | behaviorally_verified |
| MCP tool | `majordomus_shell_check` |
| HTTP | `GET /api/v1/shell/check` |
| CLI | `majordomus shell check` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::shell |
| tags | shell, governance, migration |

Input: none.

Output: `ShellReport`.

