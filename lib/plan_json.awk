# plan_json.awk — the milestones and issues of the plan model as JSON, in one pass.
#
# usage: awk -F'\t' -v kind=I|M -v flat=<flat dir> -v mermaid=<dir or empty> -f plan_json.awk model.tsv
#
# One object per M or I row of model.tsv, in row order, with every field the site
# renders: the row's own columns, the record's fields and lists from its flat file, its
# evidence entries, and for a milestone its issues, counts and (when a directory is given)
# the Mermaid graph written for it. Strings are escaped as JSON; the caller pipes the
# array through jq, which is where the pretty form comes from. This replaced one jq
# process per record plus one process per list and per column, which was most of the
# site generator's time on a plan of a hundred records.
function jstr(s,    out, i, c, n) {
    gsub(/\\/, "\\\\", s); gsub(/"/, "\\\"", s)
    gsub(/\n/, "\\n", s); gsub(/\r/, "\\r", s); gsub(/\t/, "\\t", s)
    # any other control byte, escaped as \u00XX so the value is what jq --arg would carry
    out = ""; n = length(s)
    for (i = 1; i <= n; i++) { c = substr(s, i, 1); if (c < " " && c != "") out = out sprintf("\\u%04x", index("\001\002\003\004\005\006\007\010\011\012\013\014\015\016\017\020\021\022\023\024\025\026\027\030\031\032\033\034\035\036\037", c)); else out = out c }
    return "\"" out "\""
}
function jnum(s) { return (s ~ /^-?[0-9]+$/) ? s : "0" }
function jbool(s) { return (s == "true") ? "true" : "false" }
function jcsv(s,    n, a, i, out) {  # "a,b" -> ["a","b"]; empty -> []
    if (s == "") return "[]"
    n = split(s, a, ","); out = "["
    for (i = 1; i <= n; i++) { if (a[i] == "") continue; out = out (out == "[" ? "" : ",") jstr(a[i]) }
    return out "]"
}
# the record's flat file: fv[key] = value, in file order; lists are key.N
function load(id,    f, line, eq, k) {
    delete fv; delete order; nkeys = 0
    f = flat "/" id
    while ((getline line < f) > 0) {
        eq = index(line, "="); if (eq == 0) continue
        k = substr(line, 1, eq - 1); if (k in fv) continue   # first value wins, as mj_yget did
        fv[k] = substr(line, eq + 1); order[++nkeys] = k
    }
    close(f)
}
function get(k) { return (k in fv) ? fv[k] : "" }
function jlist(k,    i, out) {  # the items of key.0, key.1 ... as a JSON array
    out = "["
    for (i = 0; (k "." i) in fv; i++) out = out (i ? "," : "") jstr(fv[k "." i])
    return out "]"
}
function jevidence(    i, out) {
    out = "["
    for (i = 0; ("evidence." i ".covers") in fv; i++)
        out = out (i ? "," : "") "{\"covers\":" jstr(get("evidence." i ".covers")) ",\"type\":" jstr(get("evidence." i ".type")) \
            ",\"command\":" jstr(get("evidence." i ".command")) ",\"result\":" jstr(get("evidence." i ".result")) \
            ",\"artifact\":" jstr(get("evidence." i ".artifact")) ",\"commit\":" jstr(get("evidence." i ".commit")) \
            ",\"recorded_at\":" jstr(get("evidence." i ".recorded_at")) "}"
    return out "]"
}
function jcounts(s,    n, a, i, kv, total, required, by, first) {  # total=1,required=1,READY=0,... -> {total,required,by_status}
    n = split(s, a, ","); by = ""; first = 1
    for (i = 1; i <= n; i++) {
        split(a[i], kv, "=")
        if (kv[1] == "total") total = kv[2]
        else if (kv[1] == "required") required = kv[2]
        else { by = by (first ? "" : ",") jstr(kv[1]) ":" jnum(kv[2]); first = 0 }
    }
    return "{\"total\":" jnum(total) ",\"required\":" jnum(required) ",\"by_status\":{" by "}}"
}
function jfile(path,    line, out, any) {  # a whole file as one JSON string, empty file -> ""
    out = ""; any = 0
    while ((getline line < path) > 0) { out = out (any ? "\n" : "") line; any = 1 }
    close(path)
    return jstr(out)
}
BEGIN { OFS = "\t"; nout = 0 }
$1 == "I" { irow[++ni] = $0; iof[$3] = iof[$3] (iof[$3] == "" ? "" : ",") $2 }
$1 == "M" { mrow[++nm] = $0 }
END {
    printf "["
    if (kind == "I") {
        for (r = 1; r <= ni; r++) {
            n = split(irow[r], c, "\t"); id = c[2]; load(id)
            printf "%s{\"id\":%s,\"slug\":%s,\"milestone\":%s,\"status\":%s,\"wave\":%s,\"priority\":%s,\"profile\":%s,\"parallel_safe\":%s,\"title\":%s,\"depends_on\":%s,\"blocked_by\":%s,\"dependents\":%s,\"scope\":%s,\"non_scope\":%s,\"objective\":%s,\"why\":%s,\"current_state\":%s,\"desired_state\":%s,\"acceptance_criteria\":%s,\"validation\":%s,\"evidence_required\":%s,\"evidence\":%s,\"risk\":%s,\"completion\":%s,\"started_at\":%s,\"verified_at\":%s,\"completed_at\":%s}", \
                (r > 1 ? "," : ""), jstr(id), jstr(c[10]), jstr(c[3]), jstr(c[4]), jnum(c[5]), jstr(c[6]), jstr(c[7]), jbool(c[8]), jstr(c[9]), \
                jcsv(c[11]), jcsv(c[12]), jcsv(c[13]), jlist("scope"), jlist("non_scope"), jstr(get("objective")), jstr(get("why")), \
                jstr(get("current_state")), jstr(get("desired_state")), jlist("acceptance_criteria"), jlist("validation"), \
                jlist("evidence_required"), jevidence(), jstr(get("risk")), jstr(get("completion")), jstr(get("started_at")), \
                jstr(get("verified_at")), jstr(get("completed_at"))
        }
    } else {
        for (r = 1; r <= nm; r++) {
            n = split(mrow[r], c, "\t"); id = c[2]; load(id)
            printf "%s{\"id\":%s,\"slug\":%s,\"status\":%s,\"order\":%s,\"priority\":%s,\"title\":%s,\"problem\":%s,\"outcome\":%s,\"current_state\":%s,\"desired_state\":%s,\"scope\":%s,\"non_scope\":%s,\"acceptance_criteria\":%s,\"validation\":%s,\"evidence_required\":%s,\"risks\":%s,\"evidence\":%s,\"issues\":%s,\"mermaid\":%s,\"counts\":%s}", \
                (r > 1 ? "," : ""), jstr(id), jstr(c[7]), jstr(c[3]), jnum(c[4]), jstr(c[5]), jstr(c[6]), jstr(get("problem")), jstr(get("outcome")), \
                jstr(get("current_state")), jstr(get("desired_state")), jlist("scope"), jlist("non_scope"), jlist("acceptance_criteria"), \
                jlist("validation"), jlist("evidence_required"), jlist("risks"), jevidence(), jcsv(iof[id]), \
                (mermaid == "" ? "\"\"" : jfile(mermaid "/" id)), jcounts(c[8])
        }
    }
    printf "]\n"
}
