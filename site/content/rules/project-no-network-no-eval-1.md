+++
title = "No network, no telemetry, no eval, no silent overwrite, no recursive deletion"
description = "No network, no telemetry, no eval, no silent overwrite, no recursive deletion"
weight = 101
[extra]
kind = "rule"
slug = "project-no-network-no-eval-1"
identity = "project.no-network-no-eval@1"
status = "active"
source = ".ai/repo/rules/project/no-network-no-eval.v1.md"
+++
{% raw %}

## Rationale

SECURITY.md states these as commitments; a commitment without a scan is a hope.

## Required behaviour

bin/, lib/, share/ and test/ contain no telemetry, no eval, no curl piped to a shell, no silent overwrite, no recursive deletion outside a temporary directory, and no network client but the one declared exception SECURITY.md names.

The exception is `majordomus context` reading the peer board of the shared MCP server this repository itself started, at the loopback URL in that server's own lease. It is one call site in `lib/context.sh`, bounded by `--max-time`, guarded to loopback, and silent on any failure. An exception is declared here and held to its shape by the scan; it is not a precedent for a second one.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

Review. test/cases/08_no_forbidden_constructs.sh scans the sources for every construct named here.
{% endraw %}
