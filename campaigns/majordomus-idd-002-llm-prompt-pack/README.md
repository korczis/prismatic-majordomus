# MJ-IDD-002 · LLM-native Majordomus mission pack

This pack is an **implementation instruction bundle**, not proof any capability has shipped. All repository-specific claims require preflight verification against actual source.

- Start: [`runbooks/OPERATOR.md`](runbooks/OPERATOR.md)
- One-shot main prompt: [`prompts/MASTER.md`](prompts/MASTER.md)
- Machine manifest: [`manifest.json`](manifest.json)
- Schema contracts: [`schemas/`](schemas/)
- Detailed mission prompts: [`prompts/`](prompts/)
- Governance loop: [`runbooks/GOVERNANCE-LOOP.md`](runbooks/GOVERNANCE-LOOP.md)
- Multi-session: [`runbooks/COOPERATION.md`](runbooks/COOPERATION.md)
- Proof structure: [`schemas/evidence.schema.json`](schemas/evidence.schema.json)

Verify: `python3 tools/validate_pack.py`. All original IDD-001 prompt contents are retained as reference material and incorporated into task-specific prompts.
