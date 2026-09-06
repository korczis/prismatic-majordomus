+++
title = "Record a decision as data, and prove the tool cannot accept it for you"
description = "Read the decisions a repository holds, validate the whole set in one command, and watch the tool refuse to write the one status only a person may write."
weight = 33
[extra]
id = "record-a-decision-before-it-is-forgotten"
source = ".ai/repo/use-cases/record-a-decision-before-it-is-forgotten.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

The decisions a repository lives by were made in conversations that are gone by the next morning, and the only record is a paragraph in a chat log nobody can find. The reasoning is lost first, so six months later the code looks arbitrary and somebody quietly undoes it.

## Outcome

The decision is a file with an identity nothing else claims, the evidence it came from, and a status that says plainly whether a person has accepted it. The tool proposes; only a person accepts, and the command line has no way to say otherwise.
