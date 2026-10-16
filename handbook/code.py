"""Highlight every code block of the embedded corpus with the site's own engine.

One highlighter on every surface (ADR 0166 §7): the site's code is highlighted by Zola, whose
engine is giallo, and the executable embeds the same giallo for the Cockpit and the reports.
The handbook renders its Markdown in the reader's browser, where neither runs, so its code
blocks are highlighted here, at build time, by Zola itself under the site's own
`[markdown.highlighting]` section and theme: the markup a block gets is the markup the site
gives it, and the colours are share/design/code.css, which the edition inlines.

The reader looks a block up by its language and its text, exactly as markdown-it hands them
to its highlight hook; a block that is not found (one this extraction read differently) is
drawn as plain text in the same frame, never by a second highlighter.

Nothing is fetched and nothing is written beside the handbook: the scratch site lives in a
temporary directory, and the result is code.b64 in the derived directory.
"""
import base64, gzip, json, os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.environ.get("HANDBOOK_ROOT") or os.path.dirname(HERE)
OUT = os.environ.get("HANDBOOK_DERIVED") or os.path.join(ROOT, "target", "handbook")

# An opening fence as CommonMark reads it: up to three spaces, then three or more backticks or
# tildes, then the info string (a backtick fence's info may not hold a backtick).
OPEN = re.compile(r"^( {0,3})(`{3,}|~{3,})(.*)$")
# Zola renders a page's content as a template, so the blocks are handed to it inside one
# `{% raw %}` block, as scripts/generate-site-data hands it the site's pages; a block that
# would close that raw block itself is not handed to Zola and is drawn as plain text.
RAW_END = re.compile(r"\{%-?\s*endraw\s*-?%\}")
# A language name giallo could know; anything else is plain text, as on the site.
LANG = re.compile(r"^[A-Za-z0-9_+#.-]+$")
# One highlighted block of Zola's output; the text inside is escaped, so it holds no </pre>.
BLOCK = re.compile(r'<pre class="giallo[^"]*"[^>]*>.*?</pre>', re.S)


def fences(text):
    """(language, content) of every fenced block, as markdown-it hands them to its hook."""
    lines = text.split("\n")
    i = 0
    while i < len(lines):
        m = OPEN.match(lines[i])
        if not m or (m.group(2)[0] == "`" and "`" in m.group(3)):
            i += 1
            continue
        indent, fence, info = len(m.group(1)), m.group(2), m.group(3).strip()
        close = re.compile(r"^ {0,3}" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}[ \t]*$")
        body, i = [], i + 1
        while i < len(lines) and not close.match(lines[i]):
            line = lines[i]
            strip = len(line) - len(line.lstrip(" "))
            body.append(line[min(strip, indent):])
            i += 1
        i += 1
        lang = info.split()[0] if info else ""
        yield lang, "".join(l + "\n" for l in body)


def highlighting_section(config):
    """The site's `[markdown.highlighting]` table, verbatim."""
    m = re.search(r"^\[markdown\.highlighting\]\n(?:(?!\[).*\n)*", config, re.M)
    if not m:
        sys.exit("code: site/config.toml has no [markdown.highlighting] section")
    return m.group(0)


def main():
    if not shutil.which("zola"):
        sys.stderr.write("code: zola is required; the handbook's code is highlighted by the site's engine\n")
        sys.exit(12)
    texts = json.loads(gzip.decompress(base64.b64decode(open(os.path.join(OUT, "corpus.b64")).read())))
    blocks, plain = {}, 0
    for text in texts:
        for lang, content in fences(text):
            if lang == "mermaid":
                continue
            if RAW_END.search(content):
                plain += 1
                continue
            blocks.setdefault(lang + "\0" + content, (lang, content))
    keys = sorted(blocks)
    config = open(os.path.join(ROOT, "site", "config.toml"), encoding="utf-8").read()
    section = highlighting_section(config)
    themes = re.findall(r'"([^"]+\.json)"', section)
    work = tempfile.mkdtemp(prefix="mj-handbook-code.")
    try:
        for theme in themes:
            os.makedirs(os.path.join(work, os.path.dirname(theme)), exist_ok=True)
            shutil.copy(os.path.join(ROOT, "site", theme), os.path.join(work, theme))
        os.makedirs(os.path.join(work, "templates"))
        os.makedirs(os.path.join(work, "content"))
        open(os.path.join(work, "config.toml"), "w", encoding="utf-8").write(
            'base_url = "https://example.invalid"\ncompile_sass = false\n'
            "build_search_index = false\n\n" + section)
        open(os.path.join(work, "templates", "index.html"), "w").write("{{ section.content | safe }}")
        parts = ["+++\n+++\n{% raw %}"]
        for key in keys:
            lang, content = blocks[key]
            longest = max([len(r) for r in re.findall(r"`+", content)] or [0])
            fence = "`" * max(3, longest + 1)
            parts.append("%s%s\n%s%s\n" % (fence, lang if LANG.match(lang) else "", content, fence))
        parts.append("{% endraw %}\n")
        open(os.path.join(work, "content", "_index.md"), "w", encoding="utf-8").write("\n".join(parts))
        done = subprocess.run(["zola", "build", "--force", "--output-dir", os.path.join(work, "out")],
                              cwd=work, capture_output=True, text=True)
        if done.returncode != 0:
            sys.stderr.write("code: zola could not highlight the corpus\n" + done.stderr[-2000:])
            sys.exit(12)
        rendered = BLOCK.findall(open(os.path.join(work, "out", "index.html"), encoding="utf-8").read())
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if len(rendered) != len(keys):
        sys.stderr.write("code: %d block(s) handed to zola, %d highlighted\n" % (len(keys), len(rendered)))
        sys.exit(12)
    table = dict(zip(keys, rendered))
    blob = json.dumps(table, ensure_ascii=False, sort_keys=True).encode("utf-8")
    gz = gzip.compress(blob, 9, mtime=0)
    open(os.path.join(OUT, "code.b64"), "w").write(base64.b64encode(gz).decode("ascii"))
    print("code", len(keys), "block(s) highlighted by zola", "plain", plain, "raw", len(blob), "gz", len(gz))


if __name__ == "__main__":
    main()
