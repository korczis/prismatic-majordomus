---
id: project.english-only
version: 1
kind: rule
title: English only
description: Code, comments, commits, documents and governance records are written in English, with no exceptions.
statement: Code, comments, commits, documents and governance records are written in English, with no exceptions.
status: active
class: blocking
depends_on: []
tags: [language]
---

# Rationale

The repository is read by people and tools that share one language; a second language in a comment or a commit message is a second rulebook for whoever cannot read it.

# Required behaviour

Code, comments, commits, documents and governance records are written in English, with no exceptions.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. 
