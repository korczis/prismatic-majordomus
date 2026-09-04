# Every prompt asset renders, and an unknown token is a failure

## What it means

A repository can keep prompt assets under `.majordomus/prompts/`, and the generated instructions point workers at them. An asset that no longer renders — an unknown token, broken front matter — is a broken reference, and is reported as a failure rather than discovered by the worker who tried to use it.

## How it works

`mj_prompt_validate` in `lib/prompt.sh` parses an asset's front matter and scans its body for template tokens, checking each against the set the renderer knows. `mj_validate_prompts` runs it over every asset; the doctrine `prompt_integrity` is blocking and enforced by `doctor` and `watch`, so the same rule produces `FAIL` in one and `DRIFT` in the other without a second implementation. `watch` additionally reports an installation with no assets at all, because the projected instructions tell workers the assets exist.

Covered: a name that does not match its filename, an empty description, an unknown front-matter key, missing front matter, a block token used inline, and any token the renderer does not define.

## How to see it

Write an asset whose body misspells a token as `{{TSAK}}`, then:

```bash
majordomus prompt show typo       # exit 10
majordomus doctor                 # FAIL prompt typo — unknown token
majordomus watch                  # DRIFT prompt typo
```

## What it does not cover

Rendering is not usefulness. An asset whose tokens all resolve and whose text is wrong passes every check here. Nothing measures whether a prompt produced a better outcome, and the project does not claim it can.

## Why it exists

Prompt assets are referenced from generated instruction files, which means a broken one is discovered by a worker mid-task, in the least recoverable place. Checking them where the rest of the installation is checked moves that discovery to `doctor`.
