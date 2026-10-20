"""Derive explorer-data.json, gates-data.json, repo-data.json and outlook-data.json from a git tree.

Run as `python3 -I derive_data.py` (standard library only). Everything is read from one
pinned commit of the repository through `git -C R show|ls-tree|log|tag`; the repository is
never written to and no working-tree file of it is read.

    python3 -I derive_data.py                 # HEAD of the tree this file is in
    python3 -I derive_data.py --rev <commit>  # another commit (used to prove the definitions)
    python3 -I derive_data.py --out <dir>     # write somewhere else, leave this directory alone

The first three files were hand-made snapshots whose generator was lost. Their shape is
consumed by widgets.js, extras2.js, extras3.js, extras5.js and extras11.js, so it is held: the
first run copies the files it is about to replace to shape/, every run compares the shape of
what it derived with that baseline, writes derive-data-report.txt, and refuses (exit 1,
nothing overwritten) when a key or a value type appeared or disappeared. outlook-data.json was
never a snapshot: its baseline in shape/ is its first derivation, and it is held the same way
for the outlook chapters (extras15.js and the captions assemble.py fills). A list that becomes
empty loses the keys of its entries, and that is a change of shape too: the day the economics
verdict has no unmet condition left, the chapter that says so is rewritten in the same change.

Field definitions, each proved by regenerating the snapshot's own commit (see the report):

explorer-data.json
  adrs    .ai/repo/adrs/NNNN-*.md, by number: n (the four digits), t title, s status, d date,
          p path.
  rules   .ai/repo/rules/**/*.md but README.md, by path: id, v (version as a string), c class,
          st status, s the front matter's `description` (not `statement`: that is what all 168
          snapshot values are) cut to 240 characters at a word, ns the directory under rules/.
  caps    site/data/registry/registry.json `.registry.builtin`, in the registry's order: the
          capabilities the executable declares (provenance `builtin`), not the declarative
          resources the index adds for every object of the layer (`.index.objects`).
  claims  docs/CLAIMS.yaml `claims`, in the file's order: id, c claim (220), s status, t test.
  ucs     .ai/repo/use-cases/*.md but README.md, by path: id, t title, s summary, c category,
          tg target, cmd the commands joined with ", ", p path.

gates-data.json (from .ai/repo/ci/gates.yaml)
  gates      id, job, a always, o on-demand, i implies, r requires, s summary, run runs.
  classes    id, p paths, g gates (the word "full" or a list).
  on_demand  the ids of the on-demand gates, in the file's order.
  `at-finish`, a class's `except` and the `shapes` section have no place in the shape the
  page reads and are left out; the report names every key that was left out.

repo-data.json (history of the pinned commit, merge commits excluded)
  daily       [day, commits] by author date in the author's own zone, every day from the first
              to the last, zero days included.
  types       the nine commonest conventional-commit types, then ["other", the rest]: rarer
              types and headers that are not `type(scope): subject`.
  typesTotal  the number of commits counted.
  subject     [length, commits] of the subject line. In UTF-8 BYTES: that is what the snapshot
              holds, though the chart's caption says characters (see SUBJECT_LENGTH_IN).
  scopes      scopes used by at least SCOPE_MIN_COMMITS commits, commonest first, ties in
              reverse alphabetical order (the order `sort | uniq -c | sort -rn` leaves).
  releases    [day, tag] for tags vX.Y.Z from FIRST_RELEASE on that the commit contains, by the
              author date of the tagged commit.
  adr         [day, ADRs] by the earliest author date of any commit, merges included, that
              touched the ADR's present path.
  proof       [state, rules]: the state `majordomus rules report` gives each rule, derived
              here from the tree (see rule_proof_states).
  jobs        [job, gates], most gates first.

outlook-data.json (what the outlook chapters measure; every number in them comes from here)
  integration  the history, merges included: commits, merges, and the same over the
               WINDOW_DAYS ending on the newest author day (last, since, windowCommits,
               windowMerges); weeks [Monday, commits, merges] by author date; windowTypes
               [type, commits] of the window's non-merge commits ("other" for a header that is
               not conventional).
  plan         site/data/generated/plan.json: total issues, status [status, issues] and
               milestones [status, milestones] in the plan's own order of statuses, the active
               milestone and the next ready issue with their titles, and telemetry: the
               TELEMETRY_MILESTONE with its status and its issues [id, status, title].
  intents      .ai/repo/project/intents/*.yaml: id, t title, c criteria, m criteria met, st
               [state, criteria] as crate::intent names the states, ms [milestone, status],
               p path. A criterion is judged from the committed ledger (see derive_intents).
  economics    docs/generated/economics.json: the verdict (publishable, statement, unmet), the
               valid pairs recorded and the pairs publication asks for, the measured context
               reduction with its interval in parts per million (JSON here carries no
               fractions), its n, resamples, budget, candidate and selected tokens, the day it
               was measured and the tokenizer; suites [id, runs, freshness], metrics
               [id, status, n] and hypotheses [id, status].
"""
import argparse
import collections
import datetime
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPOSITORY = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(HERE)
DEFAULT_REV = "HEAD"
BASELINE_DIR = "shape"
REPORT = "derive-data-report.txt"

ADRS_DIR = ".ai/repo/adrs/"
RULES_DIR = ".ai/repo/rules/"
USE_CASES_DIR = ".ai/repo/use-cases/"
CLAIMS_PATH = "docs/CLAIMS.yaml"
REGISTRY_PATH = "site/data/registry/registry.json"
GATES_PATH = ".ai/repo/ci/gates.yaml"
LEDGER_PATH = ".ai/repo/evidence/ledger.json"

RULE_SUMMARY_LIMIT = 240
CAPABILITY_SUMMARY_LIMIT = 200
CLAIM_LIMIT = 220

# The header grammar the page's own commit validator applies (widgets.js, lab-commit).
CONVENTIONAL = re.compile(r"^([a-z]+)(\(([^)]+)\))?(!)?: ")
TYPES_SHOWN = 9
OTHER_TYPES = "other"
# The snapshot lists exactly the scopes of four or more commits. The validator's message says
# "at least three times"; the data and this constant say four.
SCOPE_MIN_COMMITS = 4
# "bytes" reproduces the snapshot; "chars" is what the chart's caption and the repository's
# 100-character limit mean. They differ only for subjects with non-ASCII characters (23 of
# 2351 commits at the snapshot; the median is 72 either way).
SUBJECT_LENGTH_IN = "bytes"
# The snapshot starts at v0.5.0 although v0.1.0 to v0.3.1 are tags of the same history and
# v0.3.1 has a release record and a GitHub release. No property of the tree separates them, so
# the floor is kept as the one constant here that is a choice rather than a derivation.
FIRST_RELEASE = (0, 5, 0)

# Rule proof, as apps/majordomus-cli/src/rules/mod.rs declares it: strongest to weakest.
RULE_STATES = ["proven", "inputs unchanged", "stale", "gated", "failing", "not run", "reviewed",
               "unrunnable", "dangling", "unproven"]
# What the outlook chapters read (outlook-data.json).
PLAN_PATH = "site/data/generated/plan.json"
ECONOMICS_PATH = "docs/generated/economics.json"
INTENTS_DIR = ".ai/repo/project/intents/"
# The milestone that would record what real sessions consume; the chapter is about it.
TELEMETRY_MILESTONE = "execution-telemetry"
CONTEXT_METRIC = "context_reduction_ratio"
# The window the report's integration tax is read over, in days of author dates.
WINDOW_DAYS = 30
# An intent criterion's states, as crate::intent::IntentEvidenceState spells them.
INTENT_STATES = ["current", "stale", "failing", "not_run", "not_derivable", "unresolved"]
CASES_DIR = "test/cases/"
CRATE_TESTS_DIR = "apps/majordomus-cli/tests/"
CRATE_SRC_DIR = "apps/majordomus-cli/src/"
SUITE_RUNNER = "test/run.sh"
CRATE_RUNNER = "rust-check"
CRATE_RUNNER_TESTLESS_MODES = ("--integration", "--doc")


class DeriveError(Exception):
    """Something this script cannot derive honestly. It ends the run with its message."""


# ------------------------------------------------------------------------------------ git

def git(*args):
    """Output of one read-only git command over the repository, as bytes."""
    done = subprocess.run(["git", "-C", REPOSITORY, "-c", "core.quotePath=false", *args],
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if done.returncode != 0:
        raise DeriveError("git %s: exit %d: %s" % (" ".join(args), done.returncode,
                                                   done.stderr.decode("utf-8", "replace").strip()))
    return done.stdout


def git_text(*args):
    return git(*args).decode("utf-8")


def entries_of(commit, paths=()):
    """{path: (object id, size)} of a commit's files: all of them, or the ones named."""
    out = {}
    for entry in git("ls-tree", "-r", "-l", "-z", commit, "--", *paths).split(b"\0"):
        if entry:
            meta, path = entry.decode("utf-8").split("\t", 1)
            _mode, kind, oid, size = meta.split()
            out[path] = (oid, int(size) if kind == "blob" else None)
    return out


class Tree:
    """The tracked files of one commit: their object ids and sizes, and their bytes on demand."""

    def __init__(self, commit):
        self.commit = commit
        self.blobs = entries_of(commit)
        self.dirs = set()
        for path in self.blobs:
            while "/" in path:
                path = path.rsplit("/", 1)[0]
                self.dirs.add(path)

    def exists(self, path):
        path = path.rstrip("/")
        return path in self.blobs or path in self.dirs

    def under(self, prefix, suffix="", direct=False):
        """The tracked paths below a directory, sorted."""
        return sorted(p for p in self.blobs if p.startswith(prefix) and p.endswith(suffix)
                      and not (direct and "/" in p[len(prefix):]))

    def read(self, paths):
        """The bytes of several files from one `git show` per 200, split by the sizes ls-tree
        gave and checked against the object ids, so a split in the wrong place cannot pass."""
        out = {}
        paths = list(paths)
        for start in range(0, len(paths), 200):
            chunk = paths[start:start + 200]
            for p in chunk:
                if p not in self.blobs or self.blobs[p][1] is None:
                    raise DeriveError("%s is not a file of %s" % (p, self.commit))
            data = git("show", *["%s:%s" % (self.commit, p) for p in chunk])
            if len(data) != sum(self.blobs[p][1] for p in chunk):
                raise DeriveError("git show returned %d bytes for %d files, expected %d"
                                  % (len(data), len(chunk), sum(self.blobs[p][1] for p in chunk)))
            at = 0
            for p in chunk:
                oid, size = self.blobs[p]
                body = data[at:at + size]
                at += size
                hasher = hashlib.sha1 if len(oid) == 40 else hashlib.sha256
                if hasher(b"blob %d\0" % size + body).hexdigest() != oid:
                    raise DeriveError("%s: bytes read do not hash to %s" % (p, oid))
                out[p] = body
        return out

    def text(self, path):
        return self.read([path])[path].decode("utf-8")


# ------------------------------------------------------------------------------------ YAML
#
# A reader for exactly the YAML these files use, and a refusal of everything else. The files
# under .ai/ are written in the layer's own subset (docs/SCHEMAS.md; the executable's reader is
# apps/majordomus-cli/src/metadata/yaml.rs), which is YAML with one freedom strict YAML lacks:
# an unquoted scalar is text to the end of its line, so it may contain ": " and may start with
# a backtick. This reader accepts a line when the subset and YAML read it the same way, or when
# only the subset can read it at all; where the two would read it differently it refuses, so
# nothing here is a guess between them.
#
# Read: block mappings with plain keys, block sequences, `- key: value` entries, one-line flow
# sequences of scalars, plain, single- and double-quoted one-line scalars, comment lines and a
# " #" comment after a plain scalar. Every scalar is returned as a string (as PyYAML's
# BaseLoader does); the callers convert the few booleans and integers they need.
# Refused by name: tabs, block scalars (| and >), multi-line scalars, flow mappings, nested or
# multi-line flow sequences, anchors, aliases, tags, quoted or complex keys, document markers,
# a backslash in a double-quoted scalar and a comma in a quoted flow entry (YAML and the subset
# disagree about both), and anything after a closing quote or bracket.
#
# Two oracles check the reading on every run, and a disagreement with either ends it:
#   - PyYAML, when it happens to be importable (it is never required): every document it can
#     read must read the same; the documents it refuses must be refused for the one freedom
#     above;
#   - the executable itself: site/data/registry/registry.json carries the title and the
#     description its index read from every ADR, rule and use case of the same commit.

class YamlShapeError(DeriveError):
    pass


_YAML_KEY = re.compile(r"([A-Za-z_][A-Za-z0-9_-]*):(?:[ ]+(.*))?$")


class _Yaml:
    def __init__(self, text, where):
        self.where = where
        self.lines = []
        for number, raw in enumerate(text.split("\n"), 1):
            if "\t" in raw:
                self.fail(number, "a tab character")
            body = raw.lstrip(" ")
            if not body.strip() or body.startswith("#"):
                continue
            if raw.rstrip() in ("---", "...") or raw.startswith("%"):
                self.fail(number, "a document marker or directive")
            self.lines.append((number, len(raw) - len(body), body.rstrip()))
        self.at = 0

    def fail(self, number, what):
        raise YamlShapeError("%s:%d: YAML shape not understood: %s" % (self.where, number, what))

    def document(self):
        if not self.lines:
            return ""
        value = self.block(self.lines[0][1])
        if self.at != len(self.lines):
            self.fail(self.lines[self.at][0], "content outside the document's structure")
        return value

    @staticmethod
    def is_dash(body):
        return body == "-" or body.startswith("- ")

    def block(self, indent):
        return self.sequence(indent) if self.is_dash(self.lines[self.at][2]) else \
            self.mapping(indent)

    def nested(self, indent, same_indent_sequence):
        """The value that follows a `key:` or a bare `-` with nothing after it."""
        if self.at < len(self.lines):
            _number, ind, body = self.lines[self.at]
            if ind > indent:
                return self.block(ind)
            if ind == indent and same_indent_sequence and self.is_dash(body):
                return self.sequence(indent)
        return ""

    def no_continuation(self, indent):
        if self.at < len(self.lines) and self.lines[self.at][1] > indent:
            self.fail(self.lines[self.at][0], "a line indented under a one-line scalar "
                                              "(multi-line scalars are not read)")

    def mapping(self, indent):
        out = {}
        while self.at < len(self.lines):
            number, ind, body = self.lines[self.at]
            if ind < indent:
                break
            if ind > indent:
                self.fail(number, "unexpected indentation")
            if self.is_dash(body):
                self.fail(number, "a sequence entry where a mapping key was expected")
            m = _YAML_KEY.match(body)
            if not m:
                self.fail(number, "not `key: value` with a plain key: %r" % body[:60])
            key, rest = m.group(1), (m.group(2) or "").strip()
            if key in out:
                self.fail(number, "the key %r given twice" % key)
            self.at += 1
            if rest == "":
                out[key] = self.nested(indent, True)
            else:
                out[key] = self.scalar(rest, number)
                self.no_continuation(indent)
        return out

    def sequence(self, indent):
        out = []
        while self.at < len(self.lines):
            number, ind, body = self.lines[self.at]
            if ind > indent:
                self.fail(number, "unexpected indentation")
            if ind < indent or not self.is_dash(body):
                break
            rest = body[1:].lstrip(" ")
            column = indent + len(body) - len(rest)
            if rest == "":
                self.at += 1
                out.append(self.nested(indent, False))
            elif self.is_dash(rest):
                self.fail(number, "a sequence written inside a sequence entry's line")
            elif _YAML_KEY.match(rest):
                # `- key: value`: a mapping whose first key sits where the text starts
                self.lines[self.at] = (number, column, rest)
                out.append(self.mapping(column))
            else:
                self.at += 1
                out.append(self.scalar(rest, number))
                self.no_continuation(indent)
        return out

    def quoted(self, s, start, number):
        """(text, index after the closing quote) of the quoted scalar that starts at `start`."""
        quote, out, i = s[start], [], start + 1
        while i < len(s):
            c = s[i]
            if c == quote:
                if quote == "'" and s[i + 1:i + 2] == "'":
                    out.append("'")          # the one escape a single-quoted scalar has
                    i += 2
                    continue
                return "".join(out), i + 1
            if c == "\\" and quote == '"':
                self.fail(number, "a backslash in a double-quoted scalar (YAML reads an "
                                  "escape, the layer's reader reads the two characters)")
            out.append(c)
            i += 1
        self.fail(number, "a quoted scalar that does not end on its line")

    def plain(self, s, number, flow):
        if s[0] in "&*!|>{}[]#" or s in ("-", "?", ":") or s[:2] in ("- ", "? ", ": "):
            self.fail(number, "a scalar starting with the YAML indicator %r" % s[:2])
        if flow and (re.search(r"[\[\]{}]|\s#", s) or re.search(r":(\s|$)", s)):
            self.fail(number, "an indicator or comment inside a flow sequence: %r" % s[:60])
        return s

    def scalar(self, s, number):
        if s[0] in "'\"":
            value, end = self.quoted(s, 0, number)
            if s[end:].strip():
                self.fail(number, "text after a closing quote: %r" % s[end:][:40])
            return value
        if s[0] == "[":
            return self.flow_sequence(s, number)
        comment = re.search(r"\s+#", s)        # after a plain scalar, " #" starts a comment
        if comment:
            s = s[:comment.start()]
        return self.plain(s, number, False)

    def flow_sequence(self, s, number):
        out, i = [], 1
        if s[1:].strip() == "]":
            return out
        while True:
            while i < len(s) and s[i] == " ":
                i += 1
            if i < len(s) and s[i] in "'\"":
                value, i = self.quoted(s, i, number)
                if "," in value:
                    self.fail(number, "a comma inside a quoted flow entry (the layer's reader "
                                      "splits there, YAML does not)")
            else:
                end = i
                while end < len(s) and s[end] not in ",]":
                    end += 1
                value = s[i:end].strip()
                if not value:
                    self.fail(number, "an empty entry in a flow sequence")
                value = self.plain(value, number, True)
                i = end
            out.append(value)
            while i < len(s) and s[i] == " ":
                i += 1
            if i < len(s) and s[i] == ",":
                i += 1
            elif i < len(s) and s[i] == "]":
                break
            else:
                self.fail(number, "a flow sequence that does not close on its line")
        if s[i + 1:].strip():
            self.fail(number, "text after a closing bracket: %r" % s[i + 1:][:40])
        return out


try:                                        # an oracle when present, never a dependency
    import yaml as _pyyaml
    _ORACLE = getattr(_pyyaml, "CBaseLoader", _pyyaml.BaseLoader)
except ImportError:
    _pyyaml = _ORACLE = None
ORACLE = collections.Counter()
# What PyYAML says of the one freedom the layer's subset has over YAML, and nothing else.
_SUBSET_ONLY = ("mapping values are not allowed", "that cannot start any token")


def parse_yaml(text, where):
    value = _Yaml(text, where).document()
    if _pyyaml is not None:
        try:
            expected = _pyyaml.load(text, Loader=_ORACLE)
        except _pyyaml.YAMLError as error:
            if not any(reason in str(error) for reason in _SUBSET_ONLY):
                raise YamlShapeError("%s: PyYAML refuses what this reader accepted: %s"
                                     % (where, error))
            ORACLE["refused"] += 1
        else:
            if (expected if expected is not None else "") != value:
                raise YamlShapeError("%s: this reader and PyYAML disagree about the document"
                                     % where)
            ORACLE["equal"] += 1
    return value


def front_matter(text, where):
    if not text.startswith("---\n"):
        raise YamlShapeError("%s: no front matter" % where)
    end = text.find("\n---\n", 3)
    if end < 0:
        raise YamlShapeError("%s: front matter does not close" % where)
    data = parse_yaml(text[4:end + 1], where)
    if not isinstance(data, dict):
        raise YamlShapeError("%s: front matter is not a mapping" % where)
    return data


def text_field(data, key, where):
    if not isinstance(data, dict):
        raise DeriveError("%s: not a mapping" % where)
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise DeriveError("%s: `%s` is missing or is not a one-line text" % (where, key))
    return value


def flag_field(data, key, where):
    value = data.get(key, "false")
    if value not in ("true", "false"):
        raise DeriveError("%s: `%s` is %r, not true or false" % (where, key, value))
    return value == "true"


def list_field(data, key, where, required=False):
    value = data.get(key, None if required else [])
    if not isinstance(value, list) or not all(isinstance(v, str) and v for v in value):
        raise DeriveError("%s: `%s` is not a list of texts" % (where, key))
    return value


def shorten(text, limit, strip=""):
    """`text`, or its first `limit` characters cut back to a whole word and marked."""
    if len(text) <= limit:
        return text
    return text[:limit].rsplit(" ", 1)[0].rstrip(strip) + " …"


# ------------------------------------------------------------------------------------ explorer

def derive_adrs(tree):
    paths = [p for p in tree.under(ADRS_DIR, ".md", direct=True)
             if re.match(r"\d{4}-", p[len(ADRS_DIR):])]
    out = []
    for path, body in tree.read(paths).items():
        meta = front_matter(body.decode("utf-8"), path)
        date = text_field(meta, "date", path)
        if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", date):
            raise DeriveError("%s: date %r is not YYYY-MM-DD" % (path, date))
        out.append({"n": path[len(ADRS_DIR):][:4], "t": text_field(meta, "title", path),
                    "s": text_field(meta, "status", path), "d": date, "p": path})
    return sorted(out, key=lambda a: (a["n"], a["p"]))


def read_rules(tree):
    """Every rule's front matter, by path."""
    paths = [p for p in tree.under(RULES_DIR, ".md") if not p.endswith("/README.md")]
    return {path: front_matter(body.decode("utf-8"), path)
            for path, body in tree.read(paths).items()}


def derive_rules(rules):
    out = []
    for path in sorted(rules):
        meta = rules[path]
        version = text_field(meta, "version", path)
        if not version.isdigit():
            raise DeriveError("%s: version %r is not a number" % (path, version))
        out.append({"id": text_field(meta, "id", path), "v": version,
                    "c": text_field(meta, "class", path), "st": text_field(meta, "status", path),
                    # the description, not the statement: see the module documentation. The
                    # snapshot's cut also drops a comma left at the end; nothing else is known
                    # to have been dropped there, so nothing else is.
                    "s": shorten(text_field(meta, "description", path), RULE_SUMMARY_LIMIT, ","),
                    "p": path, "ns": path[len(RULES_DIR):].split("/", 1)[0]})
    return out


def derive_capabilities(registry):
    try:
        declared = registry["registry"]["builtin"]
    except (KeyError, TypeError):
        raise DeriveError("%s: no .registry.builtin list" % REGISTRY_PATH)
    out = []
    for cap in declared:
        where = "%s: capability %s" % (REGISTRY_PATH, cap.get("id"))
        if (cap.get("provenance") or {}).get("source") != "builtin":
            raise DeriveError("%s: listed as builtin with another provenance" % where)
        exposure = cap["exposure"]
        http = exposure.get("http")
        cli = exposure.get("cli")
        out.append({"id": cap["id"], "m": cap["module"], "k": cap["kind"], "t": cap["title"],
                    "d": shorten(cap["description"], CAPABILITY_SUMMARY_LIMIT),
                    "tool": (exposure.get("mcp") or {}).get("tool"),
                    "http": "%s %s" % (http["method"], http["path"]) if http else None,
                    "cli": " ".join(cli["path"]) if cli else None,
                    "e": cap["execution"]["effect"], "st": cap["stability"]})
        for key in ("id", "m", "k", "t", "d", "e", "st"):
            if not isinstance(out[-1][key], str) or not out[-1][key]:
                raise DeriveError("%s: `%s` is not a text" % (where, key))
    return out


def derive_claims(tree):
    data = parse_yaml(tree.text(CLAIMS_PATH), CLAIMS_PATH)
    if not isinstance(data, dict) or not isinstance(data.get("claims"), list):
        raise DeriveError("%s: no `claims` list" % CLAIMS_PATH)
    out = []
    for n, claim in enumerate(data["claims"]):
        where = "%s: claim %d" % (CLAIMS_PATH, n + 1)
        out.append({"id": text_field(claim, "id", where),
                    "c": shorten(text_field(claim, "claim", where), CLAIM_LIMIT),
                    "s": text_field(claim, "status", where),
                    "t": text_field(claim, "test", where)})
    return out


def derive_use_cases(tree):
    paths = [p for p in tree.under(USE_CASES_DIR, ".md", direct=True)
             if not p.endswith("/README.md")]
    out = []
    for path, body in sorted(tree.read(paths).items()):
        meta = front_matter(body.decode("utf-8"), path)
        out.append({"id": text_field(meta, "id", path), "t": text_field(meta, "title", path),
                    "s": text_field(meta, "summary", path),
                    "c": text_field(meta, "category", path),
                    "tg": text_field(meta, "target", path),
                    "cmd": ", ".join(list_field(meta, "commands", path, required=True)),
                    "p": path})
    return out


def check_against_index(registry, adrs, rules, use_cases):
    """Hold this script's reading of the front matter to the executable's own: the registry of
    the same commit carries, for every object of the layer, the title and description its
    index read. Returns how many texts were compared; one that differs ends the run."""
    try:
        index = {o["path"]: o for o in registry["index"]["objects"]}
    except (KeyError, TypeError):
        raise DeriveError("%s: no .index.objects list" % REGISTRY_PATH)
    pairs = [(a["p"], "title", a["t"]) for a in adrs]
    pairs += [(path, "description", meta["description"]) for path, meta in rules.items()]
    for use_case in use_cases:
        pairs += [(use_case["p"], "title", use_case["t"]),
                  (use_case["p"], "description", use_case["s"])]
    for path, field, mine in pairs:
        if path not in index:
            raise DeriveError("%s is in the tree and not in the index of %s"
                              % (path, REGISTRY_PATH))
        if index[path].get(field) != mine:
            raise DeriveError("%s: the executable read its %s as %r, this script as %r"
                              % (path, field, index[path].get(field), mine))
    return len(pairs)


# ------------------------------------------------------------------------------------ gates

GATE_KEYS = {"id", "job", "always", "on-demand", "implies", "requires", "summary", "runs"}
CLASS_KEYS = {"id", "paths", "gates"}
MODEL_KEYS = {"version", "gates", "classes"}


def derive_gates(tree, ignored):
    model = parse_yaml(tree.text(GATES_PATH), GATES_PATH)
    if not isinstance(model, dict) or model.get("version") != "1":
        raise DeriveError("%s: not a version 1 CI model" % GATES_PATH)
    for key in ("gates", "classes"):
        if not isinstance(model.get(key), list) or not model[key]:
            raise DeriveError("%s: no `%s` list" % (GATES_PATH, key))
    for key in sorted(set(model) - MODEL_KEYS):
        ignored["%s: %s" % (GATES_PATH, key)] += 1
    gates, ids = [], set()
    for gate in model["gates"]:
        where = "%s: gate %s" % (GATES_PATH, text_field(gate, "id", GATES_PATH + ": a gate"))
        gates.append({"id": text_field(gate, "id", where), "job": text_field(gate, "job", where),
                      "a": flag_field(gate, "always", where),
                      "o": flag_field(gate, "on-demand", where),
                      "i": list_field(gate, "implies", where),
                      "r": list_field(gate, "requires", where),
                      "s": text_field(gate, "summary", where),
                      "run": text_field(gate, "runs", where)})
        ids.add(gates[-1]["id"])
        for key in sorted(set(gate) - GATE_KEYS):
            ignored["%s: gates[].%s" % (GATES_PATH, key)] += 1
    classes = []
    for cls in model["classes"]:
        where = "%s: class %s" % (GATES_PATH, text_field(cls, "id", GATES_PATH + ": a class"))
        selected = cls.get("gates")
        if selected != "full":
            selected = list_field(cls, "gates", where, required=True)
        classes.append({"id": text_field(cls, "id", where),
                        "p": list_field(cls, "paths", where, required=True), "g": selected})
        for key in sorted(set(cls) - CLASS_KEYS):
            ignored["%s: classes[].%s" % (GATES_PATH, key)] += 1
    named = [g for gate in gates for g in gate["i"] + gate["r"]]
    named += [g for cls in classes if cls["g"] != "full" for g in cls["g"]]
    unknown = sorted(set(named) - ids)
    if unknown:
        raise DeriveError("%s: gates named and not declared: %s" % (GATES_PATH, unknown))
    return {"classes": classes, "gates": gates,
            "on_demand": [g["id"] for g in gates if g["o"]]}


# ------------------------------------------------------------------------------------ history

def read_history(commit):
    """(author day, subject) of every commit of the history that is not a merge."""
    out = []
    for line in git_text("log", "--no-merges", "--date=short", "--format=%ad%x09%s",
                         commit).split("\n"):
        if line:
            day, subject = line.split("\t", 1)
            out.append((day, subject))
    if not out:
        raise DeriveError("%s has no history" % commit)
    return out


def derive_daily(history):
    per_day = collections.Counter(day for day, _ in history)
    day = datetime.date.fromisoformat(min(per_day))
    last = datetime.date.fromisoformat(max(per_day))
    out = []
    while day <= last:
        out.append([day.isoformat(), per_day.get(day.isoformat(), 0)])
        day += datetime.timedelta(days=1)
    return out


def derive_types_and_scopes(history):
    types, scopes = collections.Counter(), collections.Counter()
    for _, subject in history:
        m = CONVENTIONAL.match(subject)
        if m:
            types[m.group(1)] += 1
            if m.group(3):
                scopes[m.group(3)] += 1
    shown = sorted(types.items(), key=lambda kv: (-kv[1], kv[0]))[:TYPES_SHOWN]
    rest = len(history) - sum(n for _, n in shown)
    kept = [s for s, n in scopes.items() if n >= SCOPE_MIN_COMMITS]
    kept = sorted(sorted(kept, reverse=True), key=lambda s: -scopes[s])
    return [list(kv) for kv in shown] + [[OTHER_TYPES, rest]], kept


def derive_subject_lengths(history):
    def length(subject):
        return len(subject.encode("utf-8")) if SUBJECT_LENGTH_IN == "bytes" else len(subject)
    return [list(kv) for kv in sorted(collections.Counter(length(s) for _, s in history).items())]


def derive_releases(commit):
    fmt = "%(refname:short)%09%(*authordate:short)%(authordate:short)"
    out = []
    for line in git_text("tag", "--list", "v*", "--merged", commit, "--format=" + fmt).split("\n"):
        if not line:
            continue
        tag, day = line.split("\t")
        m = re.fullmatch(r"v(\d+)\.(\d+)\.(\d+)", tag)
        if not m:
            continue                         # v0.2.0-ai-documents and its kind: not a release
        version = tuple(int(x) for x in m.groups())
        if version >= FIRST_RELEASE:
            out.append((day, version, tag))
    return [[day, tag] for day, _, tag in sorted(out)]


def derive_adr_days(commit, adrs):
    """[day, ADRs first committed that day]. A commit touches a path if it differs from any
    parent (`-m`), so an ADR that arrived in a merge's own resolution is still dated. A path's
    history is its own: a renamed ADR is dated by the commit that gave it its present name
    (`--no-renames` only spares git the pairing; the earliest day per present path is the
    same with it and without)."""
    first, day = {}, None
    for line in git_text("log", "-m", "--no-renames", "--name-only", "--date=short",
                         "--format=@%ad", commit, "--", ADRS_DIR.rstrip("/")).split("\n"):
        if line.startswith("@"):
            day = line[1:]
        elif line:
            first[line] = min(day, first.get(line, day))
    missing = [a["p"] for a in adrs if a["p"] not in first]
    if missing:
        raise DeriveError("no commit touches %s" % missing)
    return [list(kv) for kv in sorted(collections.Counter(first[a["p"]] for a in adrs).items())]


# ------------------------------------------------------------------------------------ rule proof

def meta_text(block, key):
    """A front-matter field the way the executable's `field` reads it: trimmed, and absent
    when empty or not a scalar."""
    value = block.get(key)
    if not isinstance(value, str):
        return None
    return value.strip().strip("'") or None


def meta_list(block, key):
    value = block.get(key)
    if isinstance(value, list):
        return [v.strip() for v in value if isinstance(v, str) and v.strip()]
    if isinstance(value, str) and value.strip():
        return [value.strip()]
    return []


def test_identity(path):
    """(`suite:<case>` or `crate:<binary>`, its source) for a path a runner owns, else None."""
    path = path.strip().strip("'\"")
    for prefix, suffix, runner in ((CASES_DIR, ".sh", "suite"), (CRATE_TESTS_DIR, ".rs", "crate")):
        if path.startswith(prefix):
            name = path[len(prefix):]
            if not name.endswith(suffix):
                return None
            name = name[:-len(suffix)]
            if not name or "/" in name or "\\" in name or name.startswith("."):
                return None
            return "%s:%s" % (runner, name), prefix + name + suffix
    return None


class LedgerJudge:
    """What a run recorded in the committed evidence ledger is worth at the pinned tree, after
    src/evidence/freshness.rs: the rule proof and the intents' criteria read it the same way."""

    def __init__(self, tree):
        self.tree = tree
        self.ledger = json.loads(tree.text(LEDGER_PATH))["executions"] \
            if tree.exists(LEDGER_PATH) else []
        self.contained, self.whole = {}, {}

    def latest(self, identity):
        """The ledger's run of one test identity (`suite:<case>`), or None."""
        return next((e for e in self.ledger if e["test"] == identity), None)

    def contains(self, commit):
        """Does the pinned commit contain the commit a run was recorded on? No too when this
        repository does not have that commit at all."""
        if commit not in self.contained:
            try:
                ahead = git("log", "-1", "--format=%H", "%s..%s" % (self.tree.commit, commit))
                self.contained[commit] = not ahead.strip()
            except DeriveError:
                self.contained[commit] = False
        return self.contained[commit]

    def run_state(self, execution, source, subject):
        tree = self.tree
        outcome = execution["outcome"]
        if outcome == "skip":
            return "not run"
        if outcome != "pass":
            return "failing"
        commit = execution["commit"]
        if not self.contains(commit):
            return "stale"
        then = entries_of(commit, (source, subject))
        inputs_changed = any(then.get(p) != tree.blobs.get(p) for p in (source, subject))
        ran = execution["source"]
        moved = ran in tree.blobs and \
            "sha256:" + hashlib.sha256(tree.read([ran])[ran]).hexdigest() != execution["digest"]
        if inputs_changed or moved:
            return "stale"
        if commit not in self.whole:
            then = entries_of(commit)
            self.whole[commit] = all(then.get(p) == tree.blobs.get(p)
                                     for p in set(then) | set(tree.blobs) if p != LEDGER_PATH)
        return "proven" if self.whole[commit] and execution["working_tree"] == "clean" \
            else "inputs unchanged"


def rule_proof_states(tree, rules, gates):
    """{rule path: state}: what `majordomus rules report` says of each rule, re-derived from
    the pinned tree after apps/majordomus-cli/src/rules/mod.rs (`report`) and
    src/evidence/freshness.rs (`freshness`, `aggregate`).

    A rule that states a reason nothing executable can express it is `reviewed`; one that
    names neither a validator nor a test is `unproven`; one naming a path that is not in the
    tree is `dangling`. Otherwise each named test has a state — `gated` for a check a CI gate
    runs as its own command, `unrunnable` for a path no runner owns, `not run` when the
    committed evidence ledger holds no run of it, else what the ledger's run is worth now —
    and the rule takes the weakest state among the tests that can carry proof.

    Three things differ from the executable, none of which can be read from a commit: it
    reads the working tree where this reads the commit (an untracked file exists for it and
    not here); it compares a recorded run with the working tree, this with the commit; and it
    also reads the untracked local ledger (.ai/local/evidence/ledger.json), which this cannot
    see. The snapshot's numbers (135 not run, 11 reviewed, 10 unproven, 6 gated, 3 stale at
    afbc25924a) are reproduced exactly by this reading of that commit.
    """
    rank = {state: n for n, state in enumerate(RULE_STATES)}
    commands = [(g["id"], g["run"]) for g in gates]
    lib = tree.read(tree.under("lib/", ".sh", direct=True))
    defined = [line.lstrip() for body in lib.values()
               for line in body.decode("utf-8", "replace").split("\n")
               if line.lstrip().startswith("mj_validate_")]
    judge = LedgerJudge(tree)

    def unit_test_module(path):
        if not path.startswith(CRATE_SRC_DIR) or path.rsplit("/", 1)[-1] != "tests.rs" \
                or path not in tree.blobs:
            return False
        parent = path[len(CRATE_SRC_DIR):].rsplit("/", 1)[0] if "/" in path[len(CRATE_SRC_DIR):] \
            else ""
        parents = [CRATE_SRC_DIR + parent + "/mod.rs", CRATE_SRC_DIR + parent + ".rs"] if parent \
            else [CRATE_SRC_DIR + "lib.rs", CRATE_SRC_DIR + "main.rs"]
        for source in parents:
            if source not in tree.blobs:
                continue
            under_cfg_test = False
            for line in tree.text(source).split("\n"):
                line = line.strip()
                if not line:
                    continue
                if under_cfg_test and (line == "mod tests;" or line.endswith(" mod tests;")):
                    return True
                under_cfg_test = line == "#[cfg(test)]"
        return False

    def runs_crate_tests(runs):
        for command in re.split(r"[&|;]", runs):
            words = command.split()
            for at, word in enumerate(words):
                if word.rsplit("/", 1)[-1] == CRATE_RUNNER:
                    if all(w not in CRATE_RUNNER_TESTLESS_MODES for w in words[at + 1:]):
                        return True
                    break
        return False

    out = {}
    for path, meta in rules.items():
        block = meta.get("x-majordomus")
        block = block if isinstance(block, dict) else {}
        validator = meta_text(block, "validator")
        tests = meta_list(block, "tests")
        if validator is None and not tests:
            out[path] = "reviewed" if meta_text(block, "reviewed_because") else "unproven"
            continue
        parts = []
        for test in tests:
            identity = test_identity(test)
            if unit_test_module(test):
                test_gates = [g for g, runs in commands if runs_crate_tests(runs)]
                is_gate = bool(test_gates)
            else:
                is_gate = any(test in runs.split() for _, runs in commands)
            if not tree.exists(test):
                state = "dangling"
            elif identity:
                execution = judge.latest(identity[0])
                state = judge.run_state(execution, identity[1], path) if execution else "not run"
            else:
                state = "gated" if is_gate else "unrunnable"
            parts.append((state, bool(identity) or is_gate))
        states = [s for s, _ in parts]
        if "dangling" in states:
            state = "dangling"
        elif "failing" in states:
            state = "failing"
        else:
            carrying = [s for s, carries in parts if carries] or states
            state = max(carrying, key=rank.get) if carrying else "unproven"
        if validator is not None:
            if not any(line.startswith("mj_validate_%s()" % validator) for line in defined):
                state = "dangling"
            elif not tests:
                state = max(state, "not run", key=rank.get)
        out[path] = state
    return out


# ------------------------------------------------------------------------------------ outlook

def read_all_history(commit):
    """(author day, merge) of every commit of the history, merges included."""
    out = []
    for line in git_text("log", "--date=short", "--format=%ad%x09%P", commit).split("\n"):
        if line:
            day, parents = line.split("\t", 1)
            out.append((day, len(parents.split()) > 1))
    if not out:
        raise DeriveError("%s has no history" % commit)
    return out


def derive_integration(commit, history):
    """Merge commits against all commits: over the history, over the WINDOW_DAYS that end on
    the newest author day, and by ISO week (keyed by the week's Monday). The newest author
    day is the end of the window, never the clock, so a commit derives the same figures on
    any day it is read."""
    everything = read_all_history(commit)
    last = max(day for day, _ in everything)
    since = (datetime.date.fromisoformat(last)
             - datetime.timedelta(days=WINDOW_DAYS - 1)).isoformat()
    window = [merge for day, merge in everything if day >= since]
    weeks = collections.OrderedDict()
    for day, merge in sorted(everything):
        d = datetime.date.fromisoformat(day)
        monday = (d - datetime.timedelta(days=d.weekday())).isoformat()
        commits, merges = weeks.get(monday, (0, 0))
        weeks[monday] = (commits + 1, merges + int(merge))
    types = collections.Counter()
    for day, subject in history:
        if day >= since:
            m = CONVENTIONAL.match(subject)
            types[m.group(1) if m else OTHER_TYPES] += 1
    return {"last": last, "since": since, "window": WINDOW_DAYS,
            "commits": len(everything), "merges": sum(1 for _, m in everything if m),
            "windowCommits": len(window), "windowMerges": sum(1 for m in window if m),
            "weeks": [[monday, c, m] for monday, (c, m) in weeks.items()],
            "windowTypes": [list(kv) for kv in sorted(types.items(),
                                                      key=lambda kv: (-kv[1], kv[0]))]}


def counted(items, key, order):
    """[[value, how many]] of one key, in a declared order, then any value it does not name."""
    tally = collections.Counter(i[key] for i in items)
    return [[v, tally[v]] for v in order if tally[v]] + \
        [[v, n] for v, n in sorted(tally.items()) if v not in order]


def derive_plan(plan):
    by_id = {m["id"]: m for m in plan["milestones"]}
    issue_order = plan["statuses"]["issue"]
    milestone_order = plan["statuses"]["milestone"]
    issues = {i["id"]: i for i in plan["issues"]}
    active = by_id.get(plan["active_milestone"], {})
    nxt = issues.get(plan["next_ready"], {})
    telemetry = by_id.get(TELEMETRY_MILESTONE)
    if telemetry is None:
        raise DeriveError("%s names no milestone %s" % (PLAN_PATH, TELEMETRY_MILESTONE))
    chain = [i for i in plan["issues"] if i["milestone"] == TELEMETRY_MILESTONE]
    return {"total": len(plan["issues"]), "status": counted(plan["issues"], "status", issue_order),
            "milestones": counted(plan["milestones"], "status", milestone_order),
            "active": plan["active_milestone"], "activeTitle": active.get("title", ""),
            "next": plan["next_ready"], "nextTitle": nxt.get("title", ""),
            "telemetry": {"id": TELEMETRY_MILESTONE, "status": telemetry["status"],
                          "title": telemetry["title"],
                          "issues": [[i["id"], i["status"], i["title"]] for i in chain]}}


# The keys of an intent the outlook reads. The others (statement, invariants, non_goals, ...)
# are prose the page never shows, so they are dropped before the subset reader sees the record:
# a sentence it would refuse there (a backslash in a double-quoted scalar, I3011) cannot stop
# the figures it does read.
INTENT_KEYS = ("id", "title", "satisfaction", "milestones")


def top_level_keys(text, keys):
    """`text` with only the blocks of the top-level keys named in `keys`: a top-level key line and
    every following indented, blank or comment line belong to it."""
    kept, keep = [], False
    for line in text.split("\n"):
        if line and not line.startswith((" ", "#", "-")):
            m = _YAML_KEY.match(line)
            keep = bool(m) and m.group(1) in keys
        if keep:
            kept.append(line)
    return "\n".join(kept)


def derive_intents(tree, judge, plan):
    """Each declared intent: its criteria and what the committed ledger says of each, judged
    as `intent realization` judges it (crate::intent, IntentEvidenceState): a `test` whose run
    is proven, or whose inputs are unchanged and whose run was on a clean tree, is current and
    meets its criterion; another kind of evidence is not derivable from the ledger and never
    meets one by itself. The local ledger under .ai/local/ is not part of a commit and is not
    read, so a criterion proved only on somebody's machine reads not run here."""
    status = {m["id"]: m["status"] for m in plan["milestones"]}
    out = []
    for path in tree.under(INTENTS_DIR, ".yaml", direct=True):
        doc = parse_yaml(top_level_keys(tree.text(path), INTENT_KEYS), path)
        states = []
        for criterion in doc.get("satisfaction") or []:
            kind, ref = criterion.get("evidence", ""), (criterion.get("ref") or "").strip()
            if kind != "test":
                states.append("not_derivable")
                continue
            if not ref or not tree.exists(ref):
                states.append("unresolved")
                continue
            identity = test_identity(ref)
            execution = judge.latest(identity[0]) if identity else None
            if not execution:
                states.append("not_run")
                continue
            state = judge.run_state(execution, identity[1], identity[1])
            if state == "proven" or (state == "inputs unchanged"
                                     and execution["working_tree"] == "clean"):
                states.append("current")
            else:
                states.append({"failing": "failing", "not run": "not_run"}.get(state, "stale"))
        out.append({"id": text_field(doc, "id", path), "t": text_field(doc, "title", path),
                    "c": len(states), "m": states.count("current"),
                    "st": [[s, states.count(s)] for s in INTENT_STATES if s in states],
                    "ms": [[m, status.get(m, "unknown")] for m in list_field(doc, "milestones",
                                                                             path)],
                    "p": path})
    return out


def per_million(value):
    return int(round(value * 1000000))


def derive_economics(economics):
    verdict = economics["verdict"]
    metric = {m["id"]: m for m in economics["metrics"]}
    context = metric.get(CONTEXT_METRIC)
    if context is None or context.get("value") is None:
        raise DeriveError("%s carries no measured %s" % (ECONOMICS_PATH, CONTEXT_METRIC))
    interval = context.get("interval") or {}
    measured = economics["context"]
    pairs = sum(s["pairs"]["valid"] for s in economics["suites"])
    return {"publishable": bool(verdict["publishable"]), "statement": verdict["statement"],
            "unmet": list(verdict["unmet"]),
            "validPairs": pairs, "minPairs": economics["publication"]["min_valid_pairs"],
            "context": {"ppm": per_million(context["value"]),
                        "lowPpm": per_million(interval.get("low", context["value"])),
                        "highPpm": per_million(interval.get("high", context["value"])),
                        "n": context["n"], "resamples": interval.get("resamples", 0),
                        "budget": measured["budget_tokens"],
                        "candidate": measured["candidate_tokens"],
                        "selected": measured["selected_tokens"],
                        "at": measured["measured_at"][:10],
                        "tokenizer": measured["tokenizer"]["encoding"]},
            "suites": [[s["id"], s["runs"], s["freshness"]] for s in economics["suites"]],
            "metrics": [[m["id"], m["status"], m["n"]] for m in economics["metrics"]],
            "hypotheses": [[h["id"], h["status"]] for h in economics["hypotheses"]]}


def derive_outlook(tree, commit, history, judge):
    plan = json.loads(tree.text(PLAN_PATH))
    return {"integration": derive_integration(commit, history), "plan": derive_plan(plan),
            "intents": derive_intents(tree, judge, plan),
            "economics": derive_economics(json.loads(tree.text(ECONOMICS_PATH)))}


# ------------------------------------------------------------------------------------ shape

def shape_of(value):
    """{path: set of value types}: every key and nested key of a document. Entries of a list
    share one path (`gates[].id`); a list of scalars inside a list is a tuple and keeps its
    positions (`daily[][0]`, `daily[][1]`). A key that some entries of a list carry and others
    lack has the extra type `absent`, so a key missing from one entry is seen too."""
    kinds, seen, objects = {}, collections.Counter(), collections.Counter()

    def walk(item, path, parent):
        kind = {dict: "object", list: "array", str: "text", bool: "flag", int: "number",
                type(None): "null"}.get(type(item))
        if kind is None:
            raise DeriveError("a %s at %s cannot be written as these files' JSON"
                              % (type(item).__name__, path))
        kinds.setdefault(path, set()).add(kind)
        seen[path, parent] += 1
        if isinstance(item, dict):
            objects[path] += 1
            for key, child in item.items():
                walk(child, "%s.%s" % (path, key) if path != "$" else key, path)
        elif isinstance(item, list):
            tuple_like = path.endswith("[]") and not any(isinstance(i, (dict, list)) for i in item)
            for n, child in enumerate(item):
                walk(child, "%s[%d]" % (path, n) if tuple_like else path + "[]", None)

    walk(value, "$", None)
    for (path, parent), times in seen.items():
        if parent is not None and times < objects[parent]:
            kinds[path].add("absent")
    return kinds


def compare_shapes(old, new):
    """(keys identical, no value type the old file lacks, the differences, key counts). A
    type that only the old file shows (a null that no longer occurs) is not a difference the
    page can meet; a type only the new file shows is, and a key that an entry of the new file
    lacks is a missing key."""
    old_shape, new_shape = shape_of(old), shape_of(new)
    shared = sorted(set(old_shape) & set(new_shape))
    keys = ["key only in the old file: " + p for p in sorted(set(old_shape) - set(new_shape))]
    keys += ["key only in the new file: " + p for p in sorted(set(new_shape) - set(old_shape))]
    keys += ["key missing from some entries of the new file: " + p for p in shared
             if "absent" in new_shape[p] - old_shape[p]]
    types = ["type of %s: old %s, new %s" % (p, "|".join(sorted(old_shape[p])),
                                             "|".join(sorted(new_shape[p] - {"absent"})))
             for p in shared if not new_shape[p] - {"absent"} <= old_shape[p]]
    return not keys, not types, keys + types, len(old_shape), len(new_shape)


def counts_of(name, data):
    """The countable facts of one file, in a fixed order: [(label, number)]."""
    out = [(key, len(value) if isinstance(value, list) else value)
           for key, value in data.items() if isinstance(value, (list, int))]
    if name == "explorer-data.json":
        out.append(("caps: modules", len({c["m"] for c in data["caps"]})))
    elif name == "outlook-data.json":
        out = [("integration: commits", data["integration"]["commits"]),
               ("integration: merges", data["integration"]["merges"]),
               ("integration: weeks", len(data["integration"]["weeks"])),
               ("plan: issues", data["plan"]["total"]),
               ("plan: milestones", sum(n for _, n in data["plan"]["milestones"])),
               ("plan: telemetry issues", len(data["plan"]["telemetry"]["issues"])),
               ("intents", len(data["intents"])),
               ("intents: criteria", sum(i["c"] for i in data["intents"])),
               ("intents: criteria met", sum(i["m"] for i in data["intents"])),
               ("economics: unmet conditions", len(data["economics"]["unmet"])),
               ("economics: context n", data["economics"]["context"]["n"])]
    elif name == "gates-data.json":
        out.append(("gates: always", sum(1 for g in data["gates"] if g["a"])))
        out.append(("classes: full", sum(1 for c in data["classes"] if c["g"] == "full")))
    else:
        for key, label in (("daily", "commits"), ("subject", "commits"), ("adr", "ADRs"),
                           ("proof", "rules"), ("jobs", "gates")):
            out.append(("%s: %s in total" % (key, label), sum(n for _, n in data[key])))
    return out


def remark(old, new):
    if old == new:
        return ""
    if old and not new:
        return "  << DROPPED TO ZERO: needs an explanation"
    if (old == 0) != (new == 0) or max(old, new) >= 10 * min(old, new):
        return "  << ORDER OF MAGNITUDE: needs an explanation"
    return "  (%+d)" % (new - old)


# ------------------------------------------------------------------------------------ main

FORMATS = {   # byte-for-byte the way each snapshot was written
    "explorer-data.json": lambda d: json.dumps(d, ensure_ascii=False),
    "gates-data.json": lambda d: json.dumps(d, ensure_ascii=False, separators=(",", ":")) + "\n",
    "repo-data.json": lambda d: json.dumps(d, ensure_ascii=False),
    "outlook-data.json": lambda d: json.dumps(d, ensure_ascii=False),
}


def derive(commit, ignored):
    tree = Tree(commit)
    rules = read_rules(tree)
    registry = json.loads(tree.text(REGISTRY_PATH))
    explorer = {"adrs": derive_adrs(tree), "rules": derive_rules(rules),
                "caps": derive_capabilities(registry), "claims": derive_claims(tree),
                "ucs": derive_use_cases(tree)}
    ORACLE["index"] = check_against_index(registry, explorer["adrs"], rules, explorer["ucs"])
    gates = derive_gates(tree, ignored)
    history = read_history(commit)
    types, scopes = derive_types_and_scopes(history)
    states = collections.Counter(rule_proof_states(tree, rules, gates["gates"]).values())
    jobs = collections.Counter(g["job"] for g in gates["gates"])
    repo = {"daily": derive_daily(history), "releases": derive_releases(commit), "types": types,
            "typesTotal": len(history), "subject": derive_subject_lengths(history),
            "adr": derive_adr_days(commit, explorer["adrs"]),
            "proof": [[s, n] for s, n in sorted(states.items(),
                                                key=lambda kv: (-kv[1], RULE_STATES.index(kv[0])))],
            "jobs": [list(kv) for kv in sorted(jobs.items(), key=lambda kv: (-kv[1], kv[0]))],
            "scopes": scopes}
    for name, items in (("ADR numbers", [a["n"] for a in explorer["adrs"]]),
                        ("rule paths", [r["p"] for r in explorer["rules"]]),
                        ("capability ids", [c["id"] for c in explorer["caps"]]),
                        ("claim ids", [c["id"] for c in explorer["claims"]]),
                        ("use-case ids", [u["id"] for u in explorer["ucs"]]),
                        ("gate ids", [g["id"] for g in gates["gates"]]),
                        ("class ids", [c["id"] for c in gates["classes"]])):
        twice = sorted(k for k, n in collections.Counter(items).items() if n > 1)
        if twice:
            raise DeriveError("%s declared twice: %s" % (name, twice))
    outlook = derive_outlook(tree, commit, history, LedgerJudge(tree))
    return {"explorer-data.json": explorer, "gates-data.json": gates, "repo-data.json": repo,
            "outlook-data.json": outlook}


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--rev", default=DEFAULT_REV, help="the commit to read (%(default)s)")
    parser.add_argument("--out", default=HERE, help="where the three files and the report go")
    args = parser.parse_args()
    out_dir = os.path.abspath(args.out)
    commit = git_text("log", "-1", "--format=%H", args.rev).strip()
    short = git_text("log", "-1", "--format=%h", "--abbrev=10", commit).strip()

    ignored = collections.Counter()
    new = derive(commit, ignored)

    # the baseline: the hand-made snapshot, kept the first time it is about to be replaced
    baseline_dir = os.path.join(HERE, BASELINE_DIR)
    if out_dir == HERE:
        os.makedirs(baseline_dir, exist_ok=True)
        for name in new:
            kept, current = os.path.join(baseline_dir, name), os.path.join(HERE, name)
            if not os.path.exists(kept):
                if not os.path.exists(current):
                    raise DeriveError("%s: nothing to compare with and nothing to keep" % name)
                shutil.copy2(current, kept)

    report = ["derive_data.py: %s (%s) of %s" % (short, args.rev, REPOSITORY),
              "baseline: %s/ (the hand-made snapshot, copied before its first replacement)"
              % BASELINE_DIR, ""]
    failed = False
    for name, data in new.items():
        kept = os.path.join(baseline_dir, name)
        with open(kept if os.path.exists(kept) else os.path.join(HERE, name),
                  encoding="utf-8") as handle:
            old = json.load(handle)
        keys_same, types_same, differences, old_keys, new_keys = compare_shapes(old, data)
        failed = failed or not (keys_same and types_same)
        report.append(name)
        report.append("  keys and nested keys: %s (%d old, %d new)"
                      % ("IDENTICAL" if keys_same else "DIFFERENT", old_keys, new_keys))
        report.append("  value types:          %s"
                      % ("NONE THE OLD FILE LACKS" if types_same else "DIFFERENT"))
        report.extend("    " + line for line in differences)
        report.append("  %-28s %8s %8s" % ("counts", "old", "new"))
        old_counts = dict(counts_of(name, old)) if keys_same else {}
        for label, number in counts_of(name, data):
            before = old_counts.get(label)
            report.append("  %-28s %8s %8d%s" % (label, "-" if before is None else before, number,
                                               "" if before is None else remark(before, number)))
        report.append("")
    report.append("left out of the shape, by key (how many carry it):")
    report.extend("  %s (%d)" % kv for kv in sorted(ignored.items()))
    report.extend(["", "reading of the YAML, checked on this run:",
                   "  the executable's index (%s): %d titles and descriptions of ADRs, rules "
                   "and use cases compared, all equal" % (REGISTRY_PATH, ORACLE["index"]),
                   "  PyYAML %s: %d documents read identically; %d it cannot read, each for "
                   "an unquoted scalar the layer's YAML subset reads as text"
                   % (_pyyaml.__version__, ORACLE["equal"], ORACLE["refused"]) if _pyyaml else
                   "  PyYAML: not importable, not compared",
                   "", "definitions that differ, on purpose, from the hand-made snapshot:",
                   "  ucs[].s, ucs[].t  a doubled quote inside a single-quoted YAML scalar is one"
                   " quote (the snapshot kept `repository''s`)",
                   "  claims[].c        a claim ending in a double quote keeps it (the snapshot"
                   " stripped it)",
                   "  one commit        everything is read from one commit (the snapshot mixed"
                   " three: see below)",
                   "", "proved once, on 2026-10-12, by regenerating the snapshot's own commits"
                   " with --rev:",
                   "  afbc25924a  gates-data.json byte-identical; repo-data proof and scopes"
                   " identical",
                   "  3e288a2977  explorer adrs, rules, caps identical; repo-data daily, releases,"
                   " types, typesTotal, subject, adr, jobs identical",
                   "  8d7825451f  explorer claims identical but the one closing quote; ucs"
                   " identical but the three doubled quotes",
                   "", "result: %s" % ("FAILED: the shape changed; nothing was overwritten"
                                       if failed else "the shape is identical; files written")])

    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, REPORT), "w", encoding="utf-8") as handle:
        handle.write("\n".join(report) + "\n")
    if failed:
        sys.exit("derive_data: the shape of the data changed, see %s; nothing was overwritten"
                 % os.path.join(out_dir, REPORT))
    for name, data in new.items():
        with open(os.path.join(out_dir, name), "w", encoding="utf-8") as handle:
            handle.write(FORMATS[name](data))

    explorer, gates, repo, outlook = (new[n] for n in new)
    print("derive_data %s: adrs=%d rules=%d caps=%d (%d modules) claims=%d ucs=%d | gates=%d "
          "classes=%d on_demand=%d | commits=%d days=%d releases=%d scopes=%d proof=%s | "
          "shape identical"
          % (short, len(explorer["adrs"]), len(explorer["rules"]), len(explorer["caps"]),
             len({c["m"] for c in explorer["caps"]}), len(explorer["claims"]),
             len(explorer["ucs"]), len(gates["gates"]), len(gates["classes"]),
             len(gates["on_demand"]), repo["typesTotal"], len(repo["daily"]),
             len(repo["releases"]), len(repo["scopes"]),
             ",".join("%s:%d" % (s.replace(" ", "-"), n) for s, n in repo["proof"])))
    print("derive_data %s: outlook merges=%d/%d window=%d/%d issues=%d intents=%d criteria=%d/%d "
          "telemetry=%d publishable=%s"
          % (short, outlook["integration"]["merges"], outlook["integration"]["commits"],
             outlook["integration"]["windowMerges"], outlook["integration"]["windowCommits"],
             outlook["plan"]["total"], len(outlook["intents"]),
             sum(i["m"] for i in outlook["intents"]), sum(i["c"] for i in outlook["intents"]),
             len(outlook["plan"]["telemetry"]["issues"]),
             str(outlook["economics"]["publishable"]).lower()))


if __name__ == "__main__":
    try:
        main()
    except DeriveError as error:
        sys.exit("derive_data: %s" % error)
