# Autonomous operating contract and peer-consultation protocol

## Authoritative information order

1. Running code, current Git HEAD, CI/test evidence and current environment observation (with timestamp).
2. Effective, validated `.ai/**` rules/doctrines/ADRs/knowledge and their declared precedence.
3. Verified session/decision/issue/execution state and linked provenance.
4. Secondary docs, earlier summaries and prior chats, always checked against current source.
5. Model inference and peer opinions, explicitly labeled as hypotheses until confirmed.

If documentation contradicts implementation, open a claim-drift finding and decide whether to fix source or documentation based on policy. Do not silently assume documentation or model is right. Preserve the original evidence.

## Consultation decision tree

- **No substantive ambiguity**: do the work. No debate required.
- **Reversible architecture/implementation uncertainty**: inspect tests and `.ai`, use one peer for independent critique if connected, choose evidence-backed path, record alternative.
- **Cross-subsystem impact or competing contracts**: proposal + adversarial review + verification using current available peers and/or distinct local agents; do not duplicate modifications; reconcile with a decision record. Avoid repeated debate when no new facts emerge.
- **Unavailable peers**: report unavailability, continue low-risk tasks deterministically, never claim consensus.
- **Security, privacy, credentials, externally irreversible effect, license or significant financial commitment**: require existing authorization/policy evidence. LLM agreement cannot grant it.

## Structured consultation request

Use the smallest safe projection: `decision_id`, question, known facts with precise files/SHAs/test logs, applicable `.ai` rules, constraints, options, falsification tests, allowed data classification, remaining token/time budget. Explicitly assign distinct roles. Store each answer as evidence, not as a second source of truth.

## Bounded policy

- Reuse existing peers/mesh sessions; don't launch unapproved paid providers.
- Derive budgets from the current project policy/profile. If absent, use conservative bounded local investigation and record a policy gap rather than assuming limitless money.
- Default one proposal, one independent critique and one verification round for important architecture changes, subject to policy.
- Consultation must have exit criteria: verified option, safe fallback, or specific escalation.
- Never leak secrets/raw local prompt archives to third-party providers or publish transcript context.

## Human intervention thresholds

Ask only if explicit permission is missing or a genuinely material choice remains after exploration; present decision, evidence, safe default (if any), and exact authorization required. While awaiting approval, continue independent work that does not depend on it. Do not assume authorization for destructive repo actions, protected merges, production deployment, external account/API changes or billable resources.

## Reporting

Every lane reports `known`, `verified`, `inferred`, `unsupported`, `blocked` distinctly. A rejected peer recommendation stays linked as dissent. Consensus alone is not acceptance evidence.
