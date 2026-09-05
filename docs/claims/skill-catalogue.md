# A skill is one directory under the skills section, discovered by the source class both readers share, so that adding the file is the whole registration

## What it means

A skill is `.ai/repo/skills/<id>/SKILL.md`: YAML front matter satisfying `share/schemas/skill.schema.json` over a Markdown body that is the procedure, with optional `examples/*.md` beside it. Nothing registers it. The source class `skill` in `.ai/repo/knowledge/sources.yaml` says which tracked files are skills, and that one declaration is what the shell tool, the Rust executable and the site generator all read. Adding the directory and tracking it makes the skill exist for `majordomus skills list`, for `doctor`, for the website and for MCP; removing it makes it vanish from all of them.

## How it works

`share/kinds.yaml` declares the kind `skill` (Markdown, front matter required, identity `id`, schema `skill`), and the schema carries `x-majordomus-allow: skill`, so `majordomus generate allow` writes the allow-list the shell validator reads. `lib/skills.sh` derives one catalogue from `mj_knowledge_discover` — the rows of the `skill` and `skill_example` classes, in index order — and everything else is a projection of that catalogue: `skills list|show|check`, the doctrine validator `mj_validate_skills`, and the `skills.json` and `site/content/skills/` that `scripts/generate-site-data` writes. The Rust executable indexes the same files through the same class and serves each as `majordomus://skill/<id>`, listable with `majordomus_list` by kind and readable with `majordomus_get` and `resources/read`, with no code that knows the kind exists.

## How to see it

```bash
mkdir -p .ai/repo/skills/release-notes && $EDITOR .ai/repo/skills/release-notes/SKILL.md
git add .ai/repo/skills/release-notes
majordomus skills list                                  # release-notes  active  v1  ...
majordomus skills check                                 # OK skill 1 skill(s) ...
apps/majordomus-cli/target/debug/majordomus mcp --inspect | grep skill/   # resource majordomus://skill/release-notes
scripts/generate-site-data && ls site/content/skills/   # release-notes.md
git rm -r .ai/repo/skills/release-notes && majordomus skills list          # gone everywhere
```

## What it does not cover

An untracked file is not a skill: discovery reads the git index, never the working tree, so a skill exists once it is added. Which skill a task should load is not decided here; the generated instructions tell a worker to load only the skills the task is about.

## Why it exists

The source material of this tool kept a catalogue of five hundred and forty-six agents of which one was registered with anything that ran. A skill that has to be named in a second place to exist is a skill that will exist in one place and not the other; a skill that exists because its file does cannot drift from its own inventory.
