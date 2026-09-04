# knowledge.awk — the extraction and normalisation stage of the knowledge compiler.
#
# Reads the rows lib/knowledge.sh produces for each discovered source and derives one node
# per canonical object. It is the only place that decides what a node is, what its identity
# is, and what kind it carries; nothing downstream re-derives any of the three.
#
# input (tab separated, in discovery order):
#   S  <class> <scope> <kind> <hash> <path>     one discovered source
#   F  <path> <flat-key> <value>                a flattened key of a structured source
#   D  <path> <title>                           the first level-one heading of a document
#   L  <path> <line-no> <text>                  one raw line of a line-oriented store
#   K  <path> <line-no> <target>                one inline link found outside a code fence
#   T  <path>                                   one tracked repository path
#
# output (tab separated; the caller sorts, so ordering here is irrelevant and the output
# is byte-stable for a given input):
#   N  <id> <kind> <scope> <source-path> <source-hash> <title>
#   E  <from> <to> <type> <provenance>          one reference edge
#   X  <level> <code> <subject> <message>       findings
#
# Two rules govern everything here, and both are refusals rather than features.
#
# IDENTITY IS NOT A HASH. A node id is the object's own canonical id where it has one, and
# its repository path where it does not. A content hash says whether something CHANGED; it
# can never say what something IS, because then every edit would delete a node and create a
# stranger, and every reference to it would silently point at nothing. The hash is carried
# on the node and is never part of the id.
#
# NO EDGE WITHOUT PROVENANCE. Every edge names the file, and where it has one the field or
# line, in which the relationship was observed. An edge nobody can trace back to a line is
# not a fact, it is a guess wearing a fact's clothes — and it is a guess that the person
# best placed to notice it is wrong will never see. Nothing here infers a relationship from
# prose: the repository already states thousands of them explicitly, and those are both
# free and correct.
#
# KIND COMES FROM STRUCTURE, NEVER FROM PROSE. A node's kind is decided by the source class
# it was discovered in and by fields the file explicitly declares. Nothing here reads the
# body of a document looking for a word that suggests a type. A document containing the
# word "roadmap" is not a roadmap; it is a document that mentions one. Where no rule
# applies, the kind is `unknown` and the node is still emitted — an explicit unknown is
# information, a confident wrong answer is not.

BEGIN {
    FS = "\t"; OFS = "\t"
    # Every kind this extractor can assert. A source class declaring a kind that is not
    # here is not guessed at: its nodes carry `unknown` and a finding names the class, so
    # a new source class cannot quietly acquire a made-up type.
    known = " policy profile prompt milestone issue claim document session handover checkpoint decision question doctrine implementation test "
    # The edge types are a closed set. An undeclared type is a defect rather than a new
    # vocabulary word, because a reader who cannot enumerate the relations cannot tell a
    # missing one from one that was never modelled.
    edge_types = " part_of depends_on specified_by implemented_by tested_by declares supports references "
}

# A finding the reader produced before awk saw the file passes straight through, so
# there is one findings stream rather than two that a caller has to merge.
$1 == "X" { print; next }

# Sources are indexed by ordinal, not by path. Two classes may legitimately discover the
# same file — the curated list keeps them disjoint, but the extractor must not corrupt when
# they are not. Keying by path made the second class overwrite the first's kind and made the
# path appear twice with one kind, which produced a duplicate-id failure that named the same
# file on both sides of it. Found by the case that adds an overlapping class.
$1 == "S" { n = ++nsrc
            scls[n] = $2; sscope[n] = $3; skind[n] = $4; shash[n] = $5; spath[n] = $6
            next }
$1 == "F" { key = $3; val = $4
            # a value may itself contain tabs; rejoin everything past field 4
            for (i = 5; i <= NF; i++) val = val "\t" $i
            fk[$2 SUBSEP key] = val
            fkeys[$2] = fkeys[$2] " " key
            next }
$1 == "D" { title = $3; for (i = 4; i <= NF; i++) title = title "\t" $i; h1[$2] = title; next }
$1 == "L" { nl[$2]++; ltext[$2 SUBSEP nl[$2]] = $4; lno[$2 SUBSEP nl[$2]] = $3; next }
$1 == "K" { nk[$2]++; ktarget[$2 SUBSEP nk[$2]] = $4; kline[$2 SUBSEP nk[$2]] = $3; next }
$1 == "T" { tracked[$2] = 1; mark_dirs($2); next }

END {
    for (i = 1; i <= nsrc; i++) {
        p = spath[i]; k = skind[i]
        if (index(known, " " k " ") == 0) {
            emit(node_id("unknown", p), "unknown", sscope[i], p, shash[i], "")
            if (!(scls[i] in warned)) {
                warned[scls[i]] = 1
                finding("WARN", "unknown_kind", scls[i], "source class declares kind '" k "' which this extractor has no rule for; its nodes are unknown")
            }
            continue
        }
        if      (k == "claim")    extract_claims(i)
        else if (k == "decision") extract_decisions(i)
        else if (k == "question") extract_questions(i)
        else if (k == "doctrine") extract_doctrines(i)
        else                      extract_one(i, k)
    }

    # Edges are derived only after every node exists, because an edge is emitted whether or
    # not its target resolves and the report about a dangling one has to know the difference
    # between "not extracted yet" and "not there".
    for (i = 1; i <= nsrc; i++) {
        k = skind[i]
        if      (k == "claim")    edges_claims(i)
        else if (k == "issue")    edges_issue(i)
        else if (k == "milestone") edges_milestone(i)
        else if (k == "doctrine") edges_doctrines(i)
        else if (k == "document") edges_links(i)
    }
    report_edges()
}

# ---------------------------------------------------------------- one node per file
# The common case: the file is the object. Its identity is the id it declares, or its path
# when it declares none; its title is the title it declares, or the document's own first
# heading, or nothing at all. Nothing is invented from the filename to fill a gap — an
# untitled node is an untitled node, and a reader can see that.
function extract_one(i, k,   p, id, title) {
    p = spath[i]
    id = ""
    if      (k == "milestone" || k == "issue") id = f(p, "id")
    else if (k == "profile" || k == "prompt")  id = f(p, "name")
    else if (k == "session")                   id = f(p, "session_id")
    else if (k == "handover" || k == "checkpoint") id = basename_noext(p)

    title = ""
    if      (k == "milestone" || k == "issue") title = f(p, "title")
    else if (k == "profile" || k == "prompt")  title = f(p, "description")
    else if (k == "document")                  title = h1[p]

    if (id == "") id = p
    emit(node_id(k, id), k, sscope[i], p, shash[i], title)
}

# ---------------------------------------------------------------- the claims matrix
# One file, many objects. Each claim already carries an id that the whole repository refers
# to, so that id is the node id and the claim sentence is the title.
function extract_claims(i,   p, n, id, t) {
    p = spath[i]
    for (n = 0; ; n++) {
        id = f(p, "claims." n ".id")
        if (id == "") break
        t = f(p, "claims." n ".claim")
        emit(node_id("claim", id), "claim", sscope[i], p, shash[i], t)
    }
    if (n == 0) finding("WARN", "empty_source", p, "declared as the claims matrix and yields no claim")
}

# ---------------------------------------------------------------- decisions.md
# An append-only store of dated entries whose heading is "YYYY-MM-DD — <title>". The store
# has no id field, so the identity is the title — deliberately WITHOUT the date, because
# that is exactly what the ledger's `decision.recorded` event records and what a session
# envelope references. Three places name the same thing the same way and nothing has to map
# between them.
#
# The cost is that two entries with one title collide, and the duplicate-id failure reports
# it. That is the right report rather than a nuisance: in an append-only store a repeated
# title makes every reference to it ambiguous, and `Supersedes:` exists for the case where
# one entry replaces another.
#
# The commented-out template in the shipped file is skipped: a heading inside an HTML
# comment is documentation of the format, not an entry in it.
function extract_decisions(si,   p, i, line, incomment, t) {
    p = spath[si]
    for (i = 1; i <= nl[p]; i++) {
        line = ltext[p SUBSEP i]
        if (line ~ /^[ \t]*<!--/) { incomment = 1 }
        if (incomment) { if (line ~ /-->[ \t]*$/) incomment = 0; continue }
        if (line !~ /^## /) continue
        t = substr(line, 4)
        sub(/^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9] — /, "", t)
        if (t == "") continue
        emit(node_id("decision", t), "decision", sscope[si], p, shash[si], t)
    }
}

# ---------------------------------------------------------------- open-questions.md
# The same shape, and the same rule: the identity is the question text, which is what the
# ledger records and what a session envelope references.
#
# Getting that text out has to survive resolution, and the first attempt did not. The writer
# appends " — <answer>" to the whole existing line, so an entry reads
#   - [unresolved] <task> — <question> (<date>)
# and then
#   - [resolved <date>] <task> — <question> (<date>) — <answer>
# Keying on everything after the bracket made a question change identity the moment somebody
# answered it, which is the one thing an identity must not do. The answer is stripped only
# when what remains ends in the opening date, which is the exact shape the writer produces;
# anything else is left alone rather than being cut at a guess.
#
# Resolved and unresolved are both nodes. A question that was answered is still something
# the repository decided, and dropping it would make the record of what blocked what depend
# on whether anybody had got round to answering.
function extract_questions(si,   p, i, line, incomment, t) {
    p = spath[si]
    for (i = 1; i <= nl[p]; i++) {
        line = ltext[p SUBSEP i]
        if (line ~ /^[ \t]*<!--/) { incomment = 1 }
        if (incomment) { if (line ~ /-->[ \t]*$/) incomment = 0; continue }
        if (line !~ /^- \[(unresolved|resolved [0-9-]+)\] /) continue
        t = line
        sub(/^- \[[^]]*\] /, "", t)
        t = strip_answer(t)
        sub(/^[^ ]+ — /, "", t)          # the task id: it is an attribute, not the identity
        sub(/ \([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]\)$/, "", t)
        if (t == "") continue
        emit(node_id("question", t), "question", sscope[si], p, shash[si], t)
    }
}

# Remove a trailing " — <answer>", but only when doing so leaves a line ending in the
# opening date. A question whose own text contains " — " is then left whole rather than
# truncated on a guess.
#
# The date is spelled out digit by digit rather than with an interval like {4}: interval
# expressions are not portable across the awks this project has to run on, and the floor is
# BSD userland.
function strip_answer(t,   cut, head) {
    if (t !~ / — /) return t
    cut = t
    while (cut ~ / — /) {
        head = cut
        sub(/ — [^—]*$/, "", head)
        if (head ~ / \([0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]\)$/) return head
        cut = head
    }
    return t
}

# ---------------------------------------------------------------- emit
# A duplicate id is a defect, not a last-one-wins merge: two objects claiming one identity
# means every reference to it is ambiguous, and silently keeping one of them hides that.
function emit(id, k, sc, path, h, title) {
    if (id in seen) {
        finding("FAIL", "duplicate_id", id, "claimed by " seen[id] " and by " path)
        return
    }
    seen[id] = path
    # A node whose id is <kind>:<its own path> IS that file. A file that yields many nodes —
    # the claims matrix, the doctrine registry — has no single node, and a link to it is a
    # link to a container rather than to one thing.
    if (id == k ":" path) pathnode[path] = id
    gsub(/\t/, " ", title)
    print "N", id, k, sc, path, h, title
}

function finding(level, code, subject, msg) { print "X", level, code, subject, msg }

# A node id is <kind>:<key>. The key is a canonical id, a path, or an entry heading; it is
# never a hash and never a title alone, because titles change and paths and ids do not
# change without somebody meaning them to.
function node_id(k, key) { return k ":" key }

function f(path, key) { return (path SUBSEP key) in fk ? fk[path SUBSEP key] : "" }

function basename_noext(p,   n, parts, b) {
    n = split(p, parts, "/")
    b = parts[n]
    sub(/\.[^.]*$/, "", b)
    return b
}

# ---------------------------------------------------------------- the doctrine registry
# One file, many objects, each already keyed by an id that the tool, the documentation and
# the site all refer to.
function extract_doctrines(si,   p, n, id, t) {
    p = spath[si]
    for (n = 0; ; n++) {
        id = f(p, "doctrines." n ".id")
        if (id == "") break
        t = f(p, "doctrines." n ".title")
        emit(node_id("doctrine", id), "doctrine", sscope[si], p, shash[si], t)
    }
}

# ---------------------------------------------------------------- edges
# Every edge passes through here, and it is the only way to make one. The four arguments are
# not optional: an edge missing any of them is refused and reported rather than emitted with
# a blank, because a blank provenance reads as "unknown source" and is indistinguishable
# from one nobody bothered to record.
function add_edge(from, to, type, prov) {
    if (from == "" || to == "" || type == "" || prov == "") {
        finding("FAIL", "edge_incomplete", from " -> " to,
                "type '" type "' provenance '" prov "': an edge needs all four of from, to, type and provenance")
        return
    }
    if (index(edge_types, " " type " ") == 0) {
        finding("FAIL", "edge_type", type, "is not a declared edge type")
        return
    }
    if ((from SUBSEP to SUBSEP type) in edge_seen) return
    edge_seen[from SUBSEP to SUBSEP type] = 1
    ne++
    e_from[ne] = from; e_to[ne] = to; e_type[ne] = type; e_prov[ne] = prov
    # Owned edges come from a relationship Majordomus itself defines, so a dangling one is a
    # broken promise. A link in a document is the author's, and pointing at something outside
    # the extracted set is ordinary — gating on it would make a permanently red report, which
    # is a report people stop reading.
    e_owned[ne] = (type == "references") ? 0 : 1
}

# Every ancestor of a tracked file is a tracked directory. Nothing lists directories — the
# index holds files — and a link to one is ordinary prose, so without this every
# `[the cases](test/cases)` in the documentation is reported as broken.
function mark_dirs(path,   d) {
    d = path
    while (sub(/\/[^\/]*$/, "", d)) {
        if (d == "") break
        tracked[d] = 1
    }
}

# One declared path relationship. Three outcomes, and the difference between them is the
# whole point:
#   the path is a node          -> the edge, which is what was wanted
#   the path is a tracked file
#   or directory, but not a
#   node this compiler models   -> nothing, silently. The relationship is real and the file
#                                  is real; only the model is narrower than the repository,
#                                  and reporting that on every claim would be a report that
#                                  is large by design.
#   the path is not in the
#   repository at all           -> a failure. Something declares a relationship to a file
#                                  that does not exist, and that is a broken promise rather
#                                  than a gap in the model.
function path_edge(from, path, type, prov) {
    if (path == "" || path == "-") return
    if (path in pathnode) { add_edge(from, pathnode[path], type, prov); return }
    if (path in tracked) return
    finding("FAIL", "missing_target", from, "declares " type " " path " at " prov ", and the repository does not contain it")
}

function report_edges(   i) {
    for (i = 1; i <= ne; i++) {
        print "E", e_from[i], e_to[i], e_type[i], e_prov[i]
        if (!(e_to[i] in seen)) {
            if (e_owned[i])
                finding("FAIL", "dangling_edge", e_from[i] " -> " e_to[i],
                        "declared " e_type[i] " in " e_prov[i] " and no such node exists")
            else
                finding("WARN", "unresolved_reference", e_from[i] " -> " e_to[i],
                        "referenced from " e_prov[i] " and resolves to nothing this compiler knows")
        }
    }
}

# claim -> the document that specifies it, the file that implements it, the case that proves
# it. Three relationships the claims matrix already states in three named fields.
function edges_claims(si,   p, n, id, from, v) {
    p = spath[si]
    for (n = 0; ; n++) {
        id = f(p, "claims." n ".id")
        if (id == "") break
        from = node_id("claim", id)
        path_edge(from, f(p, "claims." n ".source"),         "specified_by",   p ":claims." n ".source")
        path_edge(from, f(p, "claims." n ".implementation"),  "implemented_by", p ":claims." n ".implementation")
        path_edge(from, f(p, "claims." n ".test"),            "tested_by",      p ":claims." n ".test")
    }
}

# issue -> its milestone, and issue -> each dependency. Both are single named fields.
function edges_issue(si,   p, id, from, n, v) {
    p = spath[si]
    id = f(p, "id"); if (id == "") return
    from = node_id("issue", id)
    v = f(p, "milestone")
    if (v != "") add_edge(from, node_id("milestone", v), "part_of", p ":milestone")
    for (n = 0; ; n++) {
        v = f(p, "depends_on." n)
        if (v == "") break
        add_edge(from, node_id("issue", v), "depends_on", p ":depends_on." n)
    }
}

# milestone -> each milestone it depends on, and -> each claim it declares.
function edges_milestone(si,   p, id, from, n, v) {
    p = spath[si]
    id = f(p, "id"); if (id == "") return
    from = node_id("milestone", id)
    for (n = 0; ; n++) {
        v = f(p, "depends_on." n)
        if (v == "") break
        add_edge(from, node_id("milestone", v), "depends_on", p ":depends_on." n)
    }
    for (n = 0; ; n++) {
        v = f(p, "claims." n)
        if (v == "") break
        add_edge(from, node_id("claim", v), "declares", p ":claims." n)
    }
}

# doctrine -> the case that proves it, and -> each claim it supports.
function edges_doctrines(si,   p, n, m, id, from, v) {
    p = spath[si]
    for (n = 0; ; n++) {
        id = f(p, "doctrines." n ".id")
        if (id == "") break
        from = node_id("doctrine", id)
        path_edge(from, f(p, "doctrines." n ".test"), "tested_by", p ":doctrines." n ".test")
        for (m = 0; ; m++) {
            v = f(p, "doctrines." n ".claims." m)
            if (v == "") break
            add_edge(from, node_id("claim", v), "supports", p ":doctrines." n ".claims." m)
        }
    }
}

# document -> document, from an inline link the author wrote. The reader has already dropped
# anything inside a code fence and anything with a scheme; what arrives here is a relative
# target, which is resolved against the linking document's own directory.
function edges_links(si,   p, i, t, target, from, prov) {
    p = spath[si]
    from = node_id("document", p)
    for (i = 1; i <= nk[p]; i++) {
        t = ktarget[p SUBSEP i]
        target = resolve_rel(p, t)
        prov = p ":" kline[p SUBSEP i]
        if (target == "") continue
        if (target in pathnode) { add_edge(from, pathnode[target], "references", prov); continue }
        # The target is a real file that this compiler does not model as one node: a
        # container of many objects, or something outside the curated set. That is ordinary
        # and silent. Gating or even warning on it would produce a report that is large by
        # design, and a report that is always large is one people stop reading.
        if (target in tracked) continue
        # Nothing in the repository is there. That is a broken link, and it is the author's
        # to fix — reported, never gating, because a document may deliberately point outside
        # the repository and this compiler cannot tell the two apart from the target alone.
        finding("WARN", "broken_link", from, "links to " target ", which the repository does not contain, at " prov)
    }
}

# Resolve a relative link against the directory of the file it appears in, collapsing "."
# and "..". A target that climbs out of the repository resolves to nothing rather than to a
# path outside it.
function resolve_rel(from_path, target,   dir, n, parts, i, out, m, j) {
    dir = from_path
    if (!sub(/\/[^\/]*$/, "", dir)) dir = ""
    if (substr(target, 1, 1) == "/") return ""
    target = (dir == "") ? target : dir "/" target
    n = split(target, parts, "/")
    m = 0
    for (i = 1; i <= n; i++) {
        if (parts[i] == "" || parts[i] == ".") continue
        if (parts[i] == "..") { if (m == 0) return ""; m--; continue }
        out[++m] = parts[i]
    }
    if (m == 0) return ""
    target = out[1]
    for (j = 2; j <= m; j++) target = target "/" out[j]
    return target
}
