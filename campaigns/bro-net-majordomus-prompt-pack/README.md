# Bro-Net × Majordomus Prompt Pack

Goal: turn Bro-Net into a concrete, testable Majordomus browser-peer subsystem without rebuilding Majordomus or recreating the speculative 2025 scope.

Core architectural invariant:

> A browser is a first-class Majordomus mesh peer with explicit capabilities, permissions, observable state, provenance-backed results, and deterministic integration across CLI/API/MCP/Cockpit/tests/docs.

Use prompts in order. Each phase assumes the previous phase has landed, is tested, documented, and deployed where applicable.

Prompts:
1. 00-audit-and-architecture.md
2. 01-browser-peer-protocol.md
3. 02-extension-and-native-bridge.md
4. 03-mesh-registration-and-capabilities.md
5. 04-task-execution-and-provenance.md
6. 05-cockpit-cli-api-mcp.md
7. 06-policies-security-and-sandboxing.md
8. 07-multi-browser-coordination.md
9. 08-e2e-tests-and-enforcement.md
10. 09-docs-landing-and-demo.md
11. 10-final-integration-deploy-verification.md

Do not implement speculative features such as behavioral manipulation, global browsing-data aggregation, distributed AI training, CAPTCHA solving networks, or a public compute marketplace unless explicitly requested later.
