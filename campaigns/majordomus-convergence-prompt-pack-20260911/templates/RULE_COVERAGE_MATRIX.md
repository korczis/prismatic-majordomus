# Rule / Doctrine Coverage Matrix

| Rule ID | Class | Canonical schema valid | Validator | Invoked by canonical gate | Unit tests | Integration tests | E2E test | CLI | API | MCP | Cockpit | Docs | Last evidence | Status |
|---|---|---:|---|---|---|---|---|---|---|---|---|---|---|---|

Acceptance for a blocking core rule:

- validator exists,
- validator is actually invoked,
- failing fixture makes the canonical gate fail,
- passing fixture passes,
- bypass/negative mutation test exists where practical,
- relevant surfaces expose status/action,
- documentation is derived/linked,
- last evidence is current for landed commit.
