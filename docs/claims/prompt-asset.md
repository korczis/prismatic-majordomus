# A prompt renders against a closed set of tokens, and an unknown token is an error

## What it means

`.majordomus/prompts/<name>.md` holds a small reusable framing — how to frame a defect so the fix is proven, how to write a continuation record, how to review a diff against its claimed scope. They are versioned with the repository, provider-neutral, and rendered against the current task's state.

This is not a prompt library. Nothing ranks them, nothing selects one automatically, and nothing loads one unless asked.

## How it works

Front matter carries `name` (which must equal the filename) and a non-empty `description`; `profile` is optional and documentation only. Unknown keys are errors, as everywhere else in Majordomus.

Rendering substitutes exactly two kinds of token and no others. Inline, anywhere in a line: `TASK`, `TASK_ID`, `PROFILE`, `SCOPE`, `OWNER`, `BRANCH`, `HEAD`, `WORKING_TREE`, `REPOSITORY`, `NOW`. Block, valid only alone on a line: `OPEN_QUESTIONS`, `DECISIONS`, `CHECKPOINT`, `HANDOVER`, `CONTEXT`.

Anything else in double braces is an **error**, and so is a block token used mid-sentence — it would expand many lines into the middle of a paragraph. There is no templating language: no conditionals, no loops, no includes, no shell interpolation.

An asset whose body asks for the whole context is excluded from `context --prompt` rather than rendered, with the exclusion named: the result would be that context nested inside itself, which the budget then pays for twice.

`doctor` and `watch` validate every asset — front matter, name match, description, and every token — so a broken asset is a reported failure rather than a surprise at the moment someone needs it.

## How to see it

```bash
majordomus prompt list
majordomus prompt render debug

printf -- '---\nname: typo\ndescription: d\n---\nhello {{TSAK}}\n' > .majordomus/prompts/typo.md
majordomus prompt render typo; echo $?      # 10, naming the unknown token
majordomus doctor                            # FAIL prompt  typo — unknown token
```

## What it does not cover

Nothing sends a prompt anywhere. `render` writes to stdout; what you do with it is your business, and Majordomus makes no network call.

Nothing measures whether a prompt helped. There is no A/B comparison and no quality score.

The shipped assets are a starting point, not a recommendation. Delete them, rewrite them, or add your own; the tool only requires that whatever is there is valid.

## Why it exists

Useful framings are rediscovered every session and lost with the conversation that produced them, so each worker reinvents a slightly worse version. Keeping a few in the repository makes them reviewable, versioned and shared. Keeping the token set closed is what stops the file from quietly becoming a program.
