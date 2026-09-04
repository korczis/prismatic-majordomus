# The repository's AI layer is one directory whose manifest names every section, readable without the tool

## What it means

Everything Majordomus knows about a repository lives under `.ai/`: `.ai/manifest.yaml` names the schema and every section of the layer, `.ai/repo/` holds the tracked canonical context (policy, profiles, rules, prompts, workflows, knowledge, the project model) and `.ai/local/` holds this checkout's own state. A person or another tool can read the layer with nothing but a file browser, because the manifest says what each directory is and `.ai/README.md` says how to read it.

## How it works

`lib/init.sh` copies the layer from `share/skeleton/ai/` and seeds the vendored rule package; it creates nothing outside `.ai/` except one ignore line in `.gitignore`, and it installs no hook, no shell file and no copy of the tool. `lib/common.sh` resolves every path the commands read or write from the manifest (`mj_resolve_layout`), so a section the manifest does not name is not a section. `doctor` fails on a section the manifest names that is absent, on a manifest key nothing reads, and on anything under `.ai/local/` that is tracked.

## How to see it

```bash
majordomus init
cat .ai/manifest.yaml            # schema: ai-repository/v1, then one line per section
rm -r .ai/repo/workflows
majordomus doctor                # FAIL layout .ai/repo/workflows — named by the manifest as section 'workflows' but absent
```

## What it does not cover

The manifest names sections, not their contents: a rule file that does not parse or a profile with an unknown key is caught by the check that reads that file, not by the layout check. Nothing here says whether the context in the layer is good, only that it is where the manifest says it is.

## Why it exists

The alternative is a tool whose data can be found only by running the tool. Then every other tool, and every person without the executable, is locked out of the repository's own operating context, and the layer cannot outlive the tool that wrote it. `test/cases/01_init.sh` proves the fresh layout file by file, and `test/cases/65_tool_root_independence.sh` proves the layer is the same whichever copy of the tool reads it.
