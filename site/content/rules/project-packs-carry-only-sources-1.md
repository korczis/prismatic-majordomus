+++
title = "A pack that leaves the machine carries only committed sources"
description = "A pack that leaves the machine carries only committed sources"
weight = 127
[extra]
kind = "rule"
slug = "project-packs-carry-only-sources-1"
identity = "project.packs-carry-only-sources@1"
status = "active"
source = ".ai/repo/rules/project/packs-carry-only-sources.v1.md"
+++
{% raw %}

## Rationale

A copy of the repository that leaves the machine is read where nobody can compare it with
the original. A directory copied by hand carries what the working tree happens to hold:
build output under `target/`, a dependency tree, a nested worktree, a cache, the local half
of the AI layer, a binary that a model cannot read and a provider may refuse. A zip is not
searched by a chat model's file index at all. Both failures are silent at the far end — the
reader sees a repository, not a mistake.

So the selection is the git index, never the directory; what a profile drops is declared
once in `share/archive.yaml` and read by both the shell's archive and the executable's pack;
and the pack is proven after it is written, because a pack that was only planned correctly
is a claim about a file nobody read back.

## Required behaviour

- The file set is the git index and the content is the index's blobs. Nothing untracked is
  ever a candidate.
- A profile with `binary: drop` leaves out a file whose extension is on `binary_extensions`
  and a file whose content is not text: a NUL byte, content that is not UTF-8 (the pack), or
  git's `-text` verdict (the archive).
- A profile with `artifacts: drop` leaves out every path matching `artifact_paths`, every
  symbolic link, and (the pack) every gitlink. An `include` never re-admits one of these.
- A profile with `derived: drop` leaves out what `.gitattributes` marks `merge=derived`.
- The pack refuses to build when a selected file names its own checkout's absolute path or
  the packing account's home directory, when one file is over the shard budget, when there
  are more files than the profile allows, and when the selection is empty.
- `pack build` verifies what it wrote, and `pack verify` refuses a pack whose files do not
  carry the digests its manifest records, that holds a file the manifest does not name, or
  that carries a forbidden path, a NUL byte or a file over the budget.
- Content is carried as committed; the pack never rewrites a file.

## Failure behaviour

`majordomus pack plan` and `pack build` exit 10 with each finding named
(`pack.file_too_large`, `pack.too_many_shards`, `pack.index_too_large`, `pack.leak`,
`pack.empty`, `pack.no_limits`) and write nothing; `pack verify` exits 10 naming
`pack.tampered`, `pack.stray`, `pack.missing`, `pack.forbidden_path`, `pack.not_text`,
`pack.over_budget` or `pack.manifest`, and 12 when the manifest cannot be read, which is
never a pass. The CI gate `pack-plan` runs the plan of the `chatgpt` profile on every change
that can move it.

## Verification

```sh
majordomus pack plan chatgpt
bash test/run.sh 871_pack_for_chatgpt 130_archive
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib pack
```
{% endraw %}
