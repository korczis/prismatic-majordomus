#!/usr/bin/env python3
"""The alphabet scan of scripts/ci/english-only-check, by code point.

It lives beside the gate rather than inside it because the thing it replaced — a bracket
range in a `grep -E` pattern — is only correct when the locale agrees, and the locale does
not. GNU grep in a UTF-8 locale refuses `[Ѐ-ӿ]` outright ("Invalid collation character"),
which on the Linux runner meant the scan printed an error to stderr, matched nothing, and
the gate announced that every authored file is spelled in English. Here a range is a range
on every machine, and the gate means the same thing on both.

Usage: english-only-scan.py <files.txt> <fixtures.txt> <names-alternation>
Prints one `path<TAB>line<TAB>character` row per offending file, the first hit only.
"""
import re
import sys

# Czech and Slovak, with their capitals: c d e n r s t u z carrying a caron or a ring.
# Polish: a c e l n s z with an ogonek, acute or dot. None of these occurs in English.
LETTERS = set(
    "čČďĎěĚňŇřŘ"
    "šŠťŤůŮžŽ"
    "ąĄćĆęĘłŁ"
    "ńŃśŚźŹżŻ"
)

# Whole scripts, unambiguous on sight: Greek, Cyrillic, Hiragana and Katakana, CJK
# unified ideographs, Hangul syllables.
SCRIPTS = (
    (0x0370, 0x03FF),
    (0x0400, 0x04FF),
    (0x3040, 0x30FF),
    (0x4E00, 0x9FFF),
    (0xAC00, 0xD7A3),
)


def offending(ch):
    if ch in LETTERS:
        return True
    o = ord(ch)
    return any(lo <= o <= hi for lo, hi in SCRIPTS)


def main(argv):
    files = [l for l in open(argv[1], encoding="utf-8").read().splitlines() if l]
    fixtures = {l for l in open(argv[2], encoding="utf-8").read().splitlines() if l}
    names = argv[3] if len(argv) > 3 else ""

    # Strip the allowed proper nouns before looking, so a line that is English but for a
    # name is English. Substitution keeps the line structure, so the number reported is
    # the line number in the file as committed.
    strip = re.compile("(" + names + ")") if names else None

    for f in files:
        if f in fixtures:
            continue
        try:
            text = open(f, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        if strip:
            text = strip.sub("", text)
        for n, line in enumerate(text.splitlines(), 1):
            hit = next((c for c in line if offending(c)), None)
            if hit is not None:
                print("%s\t%d\t%s" % (f, n, hit))
                break
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
