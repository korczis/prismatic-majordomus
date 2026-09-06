# The installer verifies an artifact's digest and inspects its archive before anything is unpacked, and any failure leaves the previous installation working

## What it means

`curl … | sh` is only as good as what the script does with what it downloads. This one, in
order: resolves the machine and refuses an unsupported one by name; reads release metadata
over HTTPS and matches every value it takes out of it against the shape it must have;
checks the artifact URL against this project's own release host; verifies the SHA-256 and
the size against the metadata before extracting anything; refuses an archive containing an
absolute path, a `..` traversal, a symbolic or hard link, a device, or any entry outside
the archive's own directory; runs the unpacked tool once, from a temporary location, and
requires it to report the version that was resolved.

Only then is the launcher replaced, by a rename. Every failure before that point leaves the
previous installation exactly as it was, and says so.

## How it works

The behaviour is `share/install/install.sh.in`; the platform facts it needs are a generated
region inside it. `sudo` appears nowhere: a destination that cannot be written is named,
not elevated. There is no warn-and-continue path — a digest mismatch discards the download
and stops.

## How to see it

```bash
curl -fsSL https://korczis.github.io/prismatic-majordomus/install.sh | sh -s -- --dry-run
bash test/run.sh 85_installer
```

The case builds a real archive from the tree you are in, serves real metadata over a real
connection, installs it, and then attacks it: a corrupted digest, a truncated download, an
archive that unpacks outside its own directory, an archive carrying a symlink to
`/etc/passwd`, an executable that reports a different version, an unwritable destination,
and a release that does not exist. After each one it requires the previously installed tool
still to run and to report its own version.

## What it does not cover

Checksums bind an artifact to its metadata; they do not prove who wrote the metadata. What
remains trusted is GitHub Pages serving that metadata, GitHub Releases serving the
artifacts, and the pipeline that produced both. Signed provenance is recorded as a next
step in `docs/DISTRIBUTION.md` rather than claimed here.

## Why it exists

The failure mode of an installer is not that it fails. It is that it half-succeeds: a
partially written binary where a working one was, or an archive that unpacked something
into a directory nobody looked at. Both are prevented by the same discipline — verify
first, stage beside, swap atomically — and both are tested by doing them.
