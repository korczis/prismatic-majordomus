# M10: Machine-enforced architecture and self-hosted evidence

Locate native Majordomus rules/doctrines/guards/schema/gates/evidence pipeline. Enforce invariants through executable checks: no shadow operation inventory, no manually duplicated UI peer/task state, no unauthenticated remote mutation, no claim of strict distributed lock without fencing/authority, no unknown remote auto-trust, no secrets in responses, no unversioned wire changes, no docs mismatch, no unsupported provider assumed ready, no generated artifact drift, no false green after skipped tests.

Hook checks into canonical local verification and GitHub CI without duplicating pipelines. Create self-dogfooding scenarios that start the actual Majordomus server/MCP/mesh, query itself via multiple transports and display proof in derived UI/evidence pages. Inject into separate minimal foreign repository and assert correct isolated source identity, discovery, commands and UI, with no dependency on project-specific hardcodes. Verify hermetic fixtures and build/test portability macOS/Linux.

Gate: newly discovered operation/peer uses canonical paths; deliberate architectural violations cause a clear failing gate; required evidence recorded and linked to issue/intent and projected into UI/docs. Tests must make false-green states fail.
