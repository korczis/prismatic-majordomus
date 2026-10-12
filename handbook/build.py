#!/usr/bin/env python3
"""Build every edition of the handbook from the tree this file is in.

    python3 handbook/build.py            build each edition into target/handbook/<lang>/
    python3 handbook/build.py --check    the same, and say only whether it holds

The handbook describes this repository, so it is built from this repository and from nothing
else: the documents, decisions, rules, claims and use cases git tracks here, the graph the
executable generates (docs/generated/graph.json), the registry, the CI model and the history.
Nothing is fetched. An edition is a directory beside this file that carries an edition.toml
naming its language; its texts are written by hand, and every figure, list, graph and panel
in it is derived by the three modules here.

The build refuses, with exit 10, when the handbook and the tree have come apart:

  - a chapter names an anchor (a document, a feature, a module, a command, a decision, a rule,
    a claim) that the repository's graph no longer carries;
  - a chapter cites nothing of the repository at all;
  - a caption asks for a figure the derived data does not have;
  - the derived data changed shape under the page that reads it (handbook/shape/).

Exit: 0 built, 10 the handbook and the tree disagree, 12 the tree cannot be read.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(HERE)
DERIVED = os.environ.get("HANDBOOK_DERIVED") or os.path.join(ROOT, "target", "handbook")


def editions():
    found = []
    for name in sorted(os.listdir(HERE)):
        manifest = os.path.join(HERE, name, "edition.toml")
        if not os.path.isfile(manifest):
            continue
        text = open(manifest, encoding="utf-8").read()
        lang = re.search(r'^\s*lang\s*=\s*"([a-z-]+)"', text, re.M)
        output = re.search(r'^\s*output\s*=\s*"([^"]+)"', text, re.M)
        if not lang or lang.group(1) != name:
            sys.stderr.write("handbook: %s declares %s; an edition's directory is named for its language\n"
                             % (os.path.join(name, "edition.toml"), lang.group(1) if lang else "no language"))
            sys.exit(10)
        found.append((name, output.group(1) if output else "index.html"))
    return found


def run(label, argv, quiet):
    env = dict(os.environ, HANDBOOK_ROOT=ROOT, HANDBOOK_DERIVED=DERIVED, PYTHONDONTWRITEBYTECODE="1")
    done = subprocess.run(argv, env=env, capture_output=True, text=True)
    if done.returncode != 0:
        sys.stderr.write("handbook: %s failed\n%s%s" % (label, done.stdout[-2000:], done.stderr[-2000:]))
        sys.exit(10 if done.returncode in (1, 10) else 12)
    if not quiet:
        for line in done.stdout.splitlines():
            if not line.lstrip().startswith("unresolved"):
                print("  " + line)
    return done.stdout


def main():
    check = "--check" in sys.argv[1:]
    unknown = [a for a in sys.argv[1:] if a != "--check"]
    if unknown:
        sys.exit("handbook: unknown option %s" % unknown[0])
    if not os.path.isfile(os.path.join(ROOT, "docs", "generated", "graph.json")):
        sys.stderr.write("handbook: %s has no docs/generated/graph.json; nothing to build from\n" % ROOT)
        sys.exit(12)
    os.makedirs(DERIVED, exist_ok=True)
    py = [sys.executable, "-I"]
    run("the repository's numbers", py + [os.path.join(HERE, "derive_data.py"), "--out", DERIVED], check)
    run("the document corpus", py + [os.path.join(HERE, "corpus.py")], check)
    run("the repository graph", py + [os.path.join(HERE, "graph.py")], check)
    built = editions()
    if not built:
        sys.stderr.write("handbook: no edition here; an edition is a directory with an edition.toml\n")
        sys.exit(12)
    for lang, output in built:
        out_dir = os.path.join(DERIVED, lang)
        os.makedirs(out_dir, exist_ok=True)
        run("the %s edition" % lang, py + [os.path.join(HERE, lang, "assemble.py"), os.path.join(out_dir, output)], check)
        size = os.path.getsize(os.path.join(out_dir, output))
        print("handbook: %s edition holds: %s (%d bytes)" % (lang, os.path.relpath(os.path.join(out_dir, output), ROOT), size))
    print("handbook: %d edition(s) built from this tree; every anchor resolves" % len(built))


if __name__ == "__main__":
    main()
