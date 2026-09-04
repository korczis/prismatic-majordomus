---
id: project.no-network-no-eval
version: 1
kind: rule
title: No network, no telemetry, no eval, no silent overwrite, no recursive deletion
description: bin/, lib/, share/ and test/ contain no network client, no telemetry, no eval, no curl piped to a shell, no silent overwrite and no recursive deletion outside a temporary directory.
statement: bin/, lib/, share/ and test/ contain no network client, no telemetry, no eval, no curl piped to a shell, no silent overwrite and no recursive deletion outside a temporary directory.
status: active
class: blocking
depends_on: []
tags: [security]
---

# Rationale

SECURITY.md states these as commitments; a commitment without a scan is a hope.

# Required behaviour

bin/, lib/, share/ and test/ contain no network client, no telemetry, no eval, no curl piped to a shell, no silent overwrite and no recursive deletion outside a temporary directory.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. test/cases/08_no_forbidden_constructs.sh scans the sources for every construct named here.
