+++
title = "Install the tool in one command, on a machine with no toolchain"
description = "Paste one line, get a verified binary for this machine, and run majordomus init — no git, no Rust, no Node, no Python, no root, and no way to install an archive whose digest does not match."
weight = 3
[extra]
id = "install-the-tool-in-one-command"
source = ".ai/repo/use-cases/install-the-tool-in-one-command.md"
category = "adoption"
maturity = "described"
+++

## Situation

Someone wants to try the tool. They are not going to clone a repository, put `bin/` on
their `PATH`, and read a contributing guide first, and they should not have to: a
supervisory tool that is hard to install is a tool that supervises nothing.

The other half of the situation is the one that makes `curl | sh` usually a bad idea. The
script is unread, the archive is unverified, a failure halfway leaves a broken binary where
a working one was, and an unsupported machine gets a confusing error instead of an answer.

## What you run

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh
majordomus init
```

or, in one line:

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --init
```

The installer resolves this machine from `uname` and, on Linux, from the C library it
finds; reads the release metadata this repository publishes rather than GitHub's API;
verifies the artifact's SHA-256 and size before unpacking anything; refuses an archive that
carries an absolute path, a traversal, a link or an entry outside its own directory; runs
the unpacked tool once to prove it reports the version that was resolved; and only then
replaces the launcher, by a rename.

`--dry-run` prints every one of those decisions and changes nothing. `--version v0.2.0`
pins a release, which is the form to use in CI.

## Outcome

A working `majordomus` on the `PATH` — or the exact line to add to make it so — and a
repository one command away from being supervised. Nothing was built, nothing was elevated,
and if anything had gone wrong the installation that was there before would still be the
installation that is there now.

The platforms this works on are not written down anywhere a person maintains: they are
`share/distribution.yaml`, and the installer's table, the release build, the documentation
and this page all read it (`docs/DISTRIBUTION.md`).

## Scenario

```yaml
setup: bare
given:
  - 'a repository with no AI layer, and a machine with the tool already installed the way the installer installs it'
steps:
  - id: version
    run: ['version']
    note: 'the installed tool says which release it is; the installer refused to install an archive whose executable said anything else'
    expect:
      exit: 0
      stdout_contains: ['^majordomus [0-9]']
  - id: init
    run: ['init']
    note: 'the command the installer points at next, in the repository you want supervised'
    expect:
      exit: 0
      stdout_contains: ['next: majordomus update', 'next: majordomus doctor']
      files_exist: ['.ai/repo/policy.yaml', '.ai/manifest.yaml']
  - id: doctor
    run: ['doctor']
    note: 'the layer the installed tool created is a layer the installed tool accepts; the hook lines init printed are not in place yet, which is exactly what doctor is for'
    expect:
      exit: 12
      stdout_contains: ['^OK   layout      .ai/']
then:
  - 'nothing was installed into the project except .ai/ and the files the policy names'
  - 'the hook line init printed names the launcher, so an upgrade does not break it'
  - 'uninstalling removes the launchers and the prefix, and no repository state'
```
