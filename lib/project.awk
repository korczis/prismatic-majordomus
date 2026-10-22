# project.awk — the one engine. Reads a flattened canonical project model and derives
# everything downstream from it: dependency graph, execution waves, issue status,
# milestone status, validation findings.
#
# There is exactly one implementation of these semantics. The CLI, the site generator
# and the GitHub adapter all call this file; none of them re-derives a status of its own.
#
# input (tab separated, issues and milestones already in sorted id order):
#   P <flat-key> <value>
#   M <milestone-id> <flat-key> <value>
#   I <issue-id> <flat-key> <value>
#   X <issue-id> <stamp-field> <sha256>  the seal its transition would have written (ADR 0097)
#
# output (tab separated, first field is the record type):
#   P  name  repository  default_branch  active_milestone
#   M  id  status  order  priority  title  slug  counts  last  rank  depends  blocked_by \
#      dependents  claims
#      `last` is the last accepted issue of a DONE milestone — its DONE issue with the latest
#      completed_at, the greater id on a tie — and empty otherwise. The release that carried
#      the milestone (released_in) is that issue's merge looked up in the release records,
#      which is git's to answer and not this file's: lib/project.sh mj_pj_m_released_in.
#   S  kind  status...                  the status vocabulary, one record per record kind
#   R  from  to                          one record per milestone dependency edge, sorted
#   I  id  milestone  status  wave  priority  profile  parallel_safe  title  slug \
#      deps  blocked_by  dependents  scope  evidence_have  evidence_need
#   W  wave  id...                      one record per wave, ids space separated
#   G  from  to                          one record per dependency edge, in sorted order
#   V  level  code  subject  message     validation findings
#
# The status vocabulary is declared once, in BEGIN, and emitted as S records. Statuses are
# derived here and stored nowhere, and no surface downstream names one: the counts field is
# keyed by the declared vocabulary, so a status added here reaches the CLI, the site and the
# documentation without anybody editing them. A status this file assigns that the
# declaration does not carry is a finding, not a silent extra column.
#
# Two graphs, deliberately not one. The issue graph inside a milestone decides what a worker
# may execute next; the milestone graph above it decides which outcomes are reachable at all.
# A milestone is gated by its dependencies being DONE, which is what makes "each step is gated
# by the previous one being real" an invariant rather than a sentence.

BEGIN { FS = "\t"; OFS = "\t"
  # Declared in derivation order — the order the tests below assign them in, which is also
  # the order every surface displays them in. Adding a status here without assigning it, or
  # assigning one without declaring it, is caught: at runtime by the reconciliation in the
  # emit block, and at rest by the case that reads the assignments out of this file.
  ISTATUS = "READY BLOCKED ACTIVE VERIFY DONE CANCELLED"
  # The three stamps a transition writes, each with the seal field and the event that prove
  # it. A stamp at or after SEALED_FROM is never excused (SEALED_FROM in plan.rs).
  SEALED_FROM = "2026-10-01T00:00:00Z"
  NSTAMP = split("started_at verified_at completed_at", STAMP, " ")
  split("started_event verified_event completed_event", SEALF, " ")
  split("plan_start plan_verify plan_done", SEALEV, " ")
  split("start verify done", SEALVERB, " ")
  MSTATUS = "PLANNED BLOCKED ACTIVE VERIFY DONE CANCELLED SUPERSEDED"
  MTITLE_MAX = 255
  # Whether length() counts characters (a UTF-8 aware awk) or bytes (BWK awk, mawk): "é" is
  # two bytes, and chars() below must count it once either way.
  UTF8_LENGTH = (length("\303\251") == 1)
}

function clean(s) { gsub(/\t/, " ", s); return s }
# The number of characters in a UTF-8 string, whichever way this awk counts: in byte mode,
# continuation bytes (10xxxxxx) are not counted. Compared byte by byte, because macOS awk
# counts bytes under a UTF-8 locale yet will not match a byte range in a regular expression.
function chars(s,   n, i, c, len) {
  if (UTF8_LENGTH) return length(s)
  len = length(s); n = 0
  for (i = 1; i <= len; i++) { c = substr(s, i, 1); if (c < "\200" || c > "\277") n++ }
  return n
}

# ---------------------------------------------------------------- ingest
$1 == "P" { pkey[$2] = $3; next }
$1 == "X" { xseal[$2, $3] = $4; next }

$1 == "M" {
  id = $2; key = $3; val = $4
  if (!(id in mseen)) { mseen[id] = 1; mids[++mn] = id }
  m[id, key] = val
  if (key ~ /^evidence\.[0-9]+\.covers$/) { mev[id, val] = 1 }
  if (key ~ /^evidence_required\.[0-9]+$/) { mneed[id, ++mneedn[id]] = val }
  if (key ~ /^depends_on\.[0-9]+$/)        { mdep[id, ++mdepn[id]] = val }
  if (key ~ /^claims\.[0-9]+$/)            { mclaim[id, ++mclaimn[id]] = val }
  delimiter(id, key, val)
  next
}

$1 == "I" {
  id = $2; key = $3; val = $4
  if (!(iseen[id])) { iseen[id] = 1; iids[++inum] = id }
  it[id, key] = val
  if (key ~ /^depends_on\.[0-9]+$/) { dep[id, ++depn[id]] = val }
  if (key ~ /^scope\.[0-9]+$/)      { scp[id, ++scpn[id]] = val }
  if (key ~ /^acceptance_criteria\.[0-9]+$/) { acn[id]++ }
  if (key ~ /^validation\.[0-9]+$/)          { valn[id]++ }
  if (key ~ /^evidence_required\.[0-9]+$/)   { need[id, ++needn[id]] = val }
  if (key ~ /^evidence\.[0-9]+\.covers$/)    { have[id, val] = 1; haven[id]++ }
  if (key ~ /^unsealed_stamps\.[0-9]+$/)     { unsealed[id, val] = 1 }
  delimiter(id, key, val)
  next
}

# ---------------------------------------------------------------- derivation
END {
  # --- local facts: is this issue DONE on its own terms?
  for (i = 1; i <= inum; i++) {
    id = iids[i]
    cancelled[id] = (it[id, "cancelled"] == "true")
    covered[id] = 1
    for (k = 1; k <= needn[id]; k++) if (!((id, need[id, k]) in have)) covered[id] = 0
    # the event behind every stamp: a stamp its transition did not write moves no status
    for (k = 1; k <= NSTAMP; k++) {
      stv = it[id, STAMP[k]]
      proven[id, k] = (stv == "" || it[id, SEALF[k]] == "sha256:" xseal[id, STAMP[k]] \
        || (stv < SEALED_FROM && ((id, STAMP[k] " " stv) in unsealed)))
      stamped[id, STAMP[k]] = (stv != "" && proven[id, k])
    }
    done[id] = (!cancelled[id] && stamped[id, "completed_at"] && covered[id])
  }

  # --- edges, and the errors visible in them
  for (i = 1; i <= inum; i++) {
    id = iids[i]
    for (k = 1; k <= depn[id]; k++) {
      d = dep[id, k]
      if (d == id) { finding("FAIL", "self_dependency", id, "depends on itself"); continue }
      if (!(iseen[d])) { finding("FAIL", "unknown_dependency", id, "depends on " d ", which is not an issue"); continue }
      if ((id, d) in edge) { finding("WARN", "duplicate_dependency", id, "names " d " more than once"); continue }
      edge[id, d] = 1
      indeg[id]++
      rdep[d] = rdep[d] == "" ? id : rdep[d] "," id
      edges[++en] = d "\t" id
    }
    if (it[id, "milestone"] == "") finding("FAIL", "no_milestone", id, "names no milestone")
    else if (!(it[id, "milestone"] in mseen)) finding("FAIL", "unknown_milestone", id, "names milestone " it[id, "milestone"] ", which does not exist")
    if (acn[id] == 0 && !cancelled[id]) finding("FAIL", "no_acceptance", id, "has no acceptance criteria; an issue without one is a placeholder")
    if (valn[id] == 0 && !cancelled[id]) finding("FAIL", "no_validation", id, "names no validation command")
    if (needn[id] == 0 && !cancelled[id]) finding("WARN", "no_evidence_required", id, "requires no evidence; nothing gates its completion")
    for (k = 1; k <= NSTAMP; k++) if (!proven[id, k])
      finding("FAIL", "stamp_without_event", id, STAMP[k] " " it[id, STAMP[k]] " has no " SEALEV[k] " event: " SEALF[k] " is absent or does not seal it, and only `majordomus plan " SEALVERB[k] "` writes the stamp")
    if (stamped[id, "completed_at"] && !covered[id] && !cancelled[id]) {
      miss = ""
      for (k = 1; k <= needn[id]; k++) if (!((id, need[id, k]) in have)) miss = miss (miss == "" ? "" : ",") need[id, k]
      finding("WARN", "evidence_missing", id, "completed_at is set but evidence is missing for " miss "; status stays VERIFY")
    }
  }

  # --- waves: Kahn layering. A node enters a wave only once every dependency has left,
  #     so its wave is one past the longest path to it. Nodes that never leave are in or
  #     downstream of a cycle, and are reported rather than silently given a wave.
  for (i = 1; i <= inum; i++) { left[iids[i]] = 1; nleft++ }
  layer = 0
  while (nleft > 0) {
    fn = 0
    for (i = 1; i <= inum; i++) {
      id = iids[i]
      if (!left[id]) continue
      ok = 1
      for (k = 1; k <= depn[id]; k++) { d = dep[id, k]; if (iseen[d] && d != id && left[d]) { ok = 0; break } }
      if (ok) front[++fn] = id
    }
    if (fn == 0) {
      stuck = ""
      for (i = 1; i <= inum; i++) if (left[iids[i]]) stuck = stuck (stuck == "" ? "" : " ") iids[i]
      finding("FAIL", "cycle", "graph", "a dependency cycle prevents these issues from ever becoming ready: " stuck)
      break
    }
    line = ""
    for (k = 1; k <= fn; k++) { id = front[k]; wave[id] = layer; left[id] = 0; nleft--; line = line (line == "" ? "" : " ") id }
    waveline[layer] = line
    layer++
  }
  waves = layer

  # --- issue status
  for (i = 1; i <= inum; i++) {
    id = iids[i]
    bb = ""
    for (k = 1; k <= depn[id]; k++) { d = dep[id, k]; if (iseen[d] && d != id && !done[d]) bb = bb (bb == "" ? "" : ",") d }
    blockedby[id] = bb
    if (cancelled[id])                       st = "CANCELLED"
    else if (done[id])                       st = "DONE"
    else if (stamped[id, "completed_at"] || stamped[id, "verified_at"]) st = "VERIFY"
    else if (stamped[id, "started_at"])      st = "ACTIVE"
    else if (bb != "")                       st = "BLOCKED"
    else                                     st = "READY"
    status[id] = st
    if (bb != "" && (st == "DONE" || st == "ACTIVE" || st == "VERIFY"))
      finding("FAIL", "premature_execution", id, "is " st " while " bb " is not DONE")
    if (!left[id] && wave[id] == "" && st != "CANCELLED") wave[id] = 0
  }

  # --- the milestone graph: validate it before anything derives from it. Same rules the
  #     issue graph lives under, applied one level up: an edge to a milestone that does not
  #     exist, an edge to itself, and a cycle are each refused by name.
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    for (k = 1; k <= mdepn[id]; k++) {
      d = mdep[id, k]
      if (d == id)          { finding("FAIL", "milestone_self_dependency", id, "requires itself"); continue }
      if (!(d in mseen))    { finding("FAIL", "milestone_unknown_dependency", id, "requires " d ", which is not a milestone here"); continue }
      if ((id SUBSEP d) in medge) { finding("WARN", "milestone_duplicate_dependency", id, "requires " d " more than once"); continue }
      medge[id, d] = 1
      mindeg[id]++; mout[d, ++moutn[d]] = id
      men++; medges[men] = d "\t" id
    }
  }

  # --- roadmap rank: Kahn layering over the milestone graph. Rank is what orders the
  #     roadmap; `order` only breaks ties inside one rank. A milestone that never leaves
  #     the graph is in or downstream of a cycle and is reported, never ranked.
  for (i = 1; i <= mn; i++) { mleft[mids[i]] = 1 }
  mnleft = mn; mlayer = 0
  while (mnleft > 0) {
    fn = 0
    for (i = 1; i <= mn; i++) {
      id = mids[i]
      if (!mleft[id]) continue
      ok = 1
      for (k = 1; k <= mdepn[id]; k++) { d = mdep[id, k]; if ((id SUBSEP d) in medge && mleft[d]) { ok = 0; break } }
      if (ok) mfront[++fn] = id
    }
    if (fn == 0) break
    for (k = 1; k <= fn; k++) { id = mfront[k]; mrank[id] = mlayer; mleft[id] = 0; mnleft-- }
    mlayer++
  }
  if (mnleft > 0) {
    cyc = ""
    for (i = 1; i <= mn; i++) if (mleft[mids[i]]) cyc = cyc (cyc == "" ? "" : " ") mids[i]
    finding("FAIL", "milestone_cycle", cyc, "milestone dependencies form a cycle; no roadmap order exists")
    for (i = 1; i <= mn; i++) if (mleft[mids[i]]) mrank[mids[i]] = 0
  }

  # --- milestone status
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    tot = 0
    delete cnt
    for (j = 1; j <= inum; j++) {
      iid = iids[j]
      if (it[iid, "milestone"] != id) continue
      tot++
      cnt[status[iid]]++
    }
    nd = cnt["DONE"] + 0; nr = cnt["READY"] + 0; na = cnt["ACTIVE"] + 0
    nv = cnt["VERIFY"] + 0; nc = cnt["CANCELLED"] + 0
    mcov = 1
    for (k = 1; k <= mneedn[id]; k++) if (!((id, mneed[id, k]) in mev)) mcov = 0
    req = tot - nc
    if (m[id, "cancelled"] == "true")                       ms = "CANCELLED"
    else if (m[id, "superseded_by"] != "")                  ms = "SUPERSEDED"
    else if (tot == 0 && mneedn[id] > 0 && mcov)             ms = "DONE"
    else if (tot == 0)                                      ms = "PLANNED"
    else if (req > 0 && nd == req && mcov)                  ms = "DONE"
    else if (req > 0 && nd == req)                          ms = "VERIFY"
    else if (na > 0 || nv > 0 || nd > 0)                    ms = "ACTIVE"
    else if (nr == 0)                                       ms = "BLOCKED"
    else                                                    ms = "PLANNED"
    mstatus[id] = ms
    mrow[id] = mcounts(id)
    mempty[id] = (tot == 0)
    if (req > 0 && nd == req && !mcov) {
      miss = ""
      for (k = 1; k <= mneedn[id]; k++) if (!((id, mneed[id, k]) in mev)) miss = miss (miss == "" ? "" : ",") mneed[id, k]
      finding("WARN", "milestone_evidence_missing", id, "every issue is DONE but milestone evidence is missing for " miss "; status stays VERIFY")
    }
  }

  # --- the gate. A milestone whose required outcomes are not yet real cannot be executable,
  #     whatever its own issues say. This is "each step is gated by the previous one being
  #     real" as an invariant rather than a sentence. It runs in rank order, so a dependency
  #     is always decided before the milestone requiring it and the block cascades.
  for (r = 0; r <= mlayer; r++) {
    for (i = 1; i <= mn; i++) {
      id = mids[i]
      if (mrank[id] != r) continue
      mblock = ""
      for (k = 1; k <= mdepn[id]; k++) {
        d = mdep[id, k]
        if (!((id SUBSEP d) in medge)) continue
        if (mstatus[d] != "DONE") mblock = mblock (mblock == "" ? "" : ",") d
      }
      mblockedby[id] = mblock
      if (mblock == "") continue
      if (mstatus[id] == "CANCELLED" || mstatus[id] == "SUPERSEDED") continue
      if (mstatus[id] == "DONE" || mstatus[id] == "VERIFY" || mstatus[id] == "ACTIVE")
        finding("FAIL", "milestone_premature", id, "is " mstatus[id] " while " mblock " is not DONE")
      mstatus[id] = "BLOCKED"
    }
  }

  for (i = 1; i <= mn; i++) {
    id = mids[i]
    if (!mempty[id]) continue
    if (mstatus[id] == "DONE" || mstatus[id] == "CANCELLED" || mstatus[id] == "SUPERSEDED") continue
    if (mblockedby[id] != "") continue
    finding("WARN", "empty_milestone", id, "is reachable and has no issues; it is an outcome nobody is executing")
  }

  # --- the gate reaches the work. An issue inside a milestone the gate has blocked is not
  #     executable, whatever its own dependencies say: the outcome it belongs to is not
  #     reachable yet. Without this the issue graph offers READY work inside a blocked
  #     milestone, and the only way to stop it is to chain issues across milestone
  #     boundaries by hand — which encodes in data a fact the graph already knows, and
  #     flattens the waves into a line so they carry no parallelism.
  for (j = 1; j <= inum; j++) {
    iid = iids[j]; mid = it[iid, "milestone"]
    if (mid == "" || mblockedby[mid] == "") continue
    if (status[iid] == "CANCELLED" || status[iid] == "DONE") continue
    if (status[iid] == "ACTIVE" || status[iid] == "VERIFY")
      finding("FAIL", "premature_execution", iid, "is " status[iid] " while its milestone " mid " waits on " mblockedby[mid])
    status[iid] = "BLOCKED"
    blockedby[iid] = (blockedby[iid] == "" ? "milestone:" mid : blockedby[iid] ",milestone:" mid)
  }

  # counts follow the statuses, so a blocked milestone cannot report ready work
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    if (mblockedby[id] == "") continue
    mrow[id] = mcounts(id)
  }

  # --- scope conflict: two issues that could run together but touch the same paths.
  #     Reported conservatively — an uncertain overlap serialises rather than races.
  for (i = 1; i <= inum; i++) for (j = i + 1; j <= inum; j++) {
    a = iids[i]; b = iids[j]
    if (status[a] != "READY" && status[a] != "ACTIVE") continue
    if (status[b] != "READY" && status[b] != "ACTIVE") continue
    if (wave[a] != wave[b]) continue
    for (x = 1; x <= scpn[a]; x++) for (y = 1; y <= scpn[b]; y++)
      if (overlap(scp[a, x], scp[b, y])) {
        finding("WARN", "scope_conflict", a, "shares " scp[a, x] " with " b "; they may not run concurrently")
        x = scpn[a] + 1; break
      }
  }

  # --- the active milestone: the lowest-ranked unblocked milestone that is ACTIVE, else the
  #     lowest-ranked unblocked one not finished. Rank comes from the milestone graph and
  #     `order` only breaks ties inside a rank, so the roadmap can never nominate a milestone
  #     whose prerequisites are not real. Derived; it can never contradict what is underneath.
  best = ""; bestord = ""
  for (pass = 1; pass <= 2; pass++) {
    for (i = 1; i <= mn; i++) {
      id = mids[i]
      if (mstatus[id] == "DONE" || mstatus[id] == "CANCELLED") continue
      if (pass == 1 && mstatus[id] != "ACTIVE") continue
      if (mblockedby[id] != "") continue
      o = (mrank[id] + 0) * 1000000 + (m[id, "order"] + 0)
      if (best == "" || o < bestord) { best = id; bestord = o }
    }
    if (best != "") break
  }

  # --- reconciliation: a status this file assigned but did not declare would reach a
  #     surface as a count nobody renders and a label nobody has a colour for. Reported by
  #     name rather than tolerated, in both directions the data can show.
  nsv = split(ISTATUS, sv, " "); delete decl
  for (v = 1; v <= nsv; v++) decl[sv[v]] = 1
  for (i = 1; i <= inum; i++)
    if (!(status[iids[i]] in decl))
      finding("FAIL", "undeclared_status", iids[i], "is " status[iids[i]] ", which the issue status vocabulary does not declare")
  nsv = split(MSTATUS, sv, " "); delete decl
  for (v = 1; v <= nsv; v++) decl[sv[v]] = 1
  for (i = 1; i <= mn; i++)
    if (!(mstatus[mids[i]] in decl))
      finding("FAIL", "undeclared_status", mids[i], "is " mstatus[mids[i]] ", which the milestone status vocabulary does not declare")

  # A milestone is projected to GitHub as "<id> — <title>" (scripts/github-sync title_of), and
  # GitHub refuses a milestone title over MTITLE_MAX characters with HTTP 422. One such record
  # turned master's github-projection red (run 39392245555, 282 characters), so the plan
  # refuses it before it is ever projected (I2844; plan.rs MILESTONE_TITLE_MAX is the same).
  for (i = 1; i <= mn; i++) {
    id = mids[i]; tl = chars(id) + 3 + chars(m[id, "title"])
    if (tl > MTITLE_MAX)
      finding("FAIL", "milestone_title_too_long", id, "projects to GitHub as a " tl "-character title (\"" id " — <title>\"); GitHub's limit is " MTITLE_MAX)
  }

  # ---------------------------------------------------------------- emit
  print "P", clean(pkey["name"]), clean(pkey["repository"]), clean(pkey["default_branch"]), best
  print "S", "issue", ISTATUS
  print "S", "milestone", MSTATUS
  # the last accepted issue of every DONE milestone, which is what a release carried last
  delete mlast
  for (j = 1; j <= inum; j++) {
    iid = iids[j]; mid = it[iid, "milestone"]
    if (status[iid] != "DONE" || !(mid in mstatus) || mstatus[mid] != "DONE") continue
    c = it[iid, "completed_at"]
    if (!(mid in mlast) || c > mlastat[mid] || (c == mlastat[mid] && iid > mlast[mid])) { mlast[mid] = iid; mlastat[mid] = c }
  }
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    mdl = ""; for (k = 1; k <= mdepn[id]; k++) if ((id SUBSEP mdep[id, k]) in medge) mdl = mdl (mdl == "" ? "" : ",") mdep[id, k]
    mcl = ""; for (k = 1; k <= mclaimn[id]; k++) mcl = mcl (mcl == "" ? "" : ",") mclaim[id, k]
    mrd = ""; for (k = 1; k <= moutn[id]; k++) mrd = mrd (mrd == "" ? "" : ",") mout[id, k]
    print "M", id, mstatus[id], m[id, "order"], m[id, "priority"], clean(m[id, "title"]), m[id, "slug"], mrow[id], \
          mlast[id], mrank[id] + 0, mdl, mblockedby[id], mrd, mcl
  }
  for (i = 1; i <= inum; i++) {
    id = iids[i]
    dl = ""; for (k = 1; k <= depn[id]; k++) dl = dl (dl == "" ? "" : ",") dep[id, k]
    sl = ""; for (k = 1; k <= scpn[id]; k++) sl = sl (sl == "" ? "" : ",") scp[id, k]
    ps = (it[id, "parallel_safe"] == "false") ? "false" : "true"
    print "I", id, it[id, "milestone"], status[id], wave[id] + 0, it[id, "priority"], it[id, "profile"], ps, \
          clean(it[id, "title"]), it[id, "slug"], dl, blockedby[id], rdep[id], sl, haven[id] + 0, needn[id] + 0
  }
  for (w = 0; w < waves; w++) if (waveline[w] != "") print "W", w, waveline[w]
  n = asortish(edges, en)
  for (k = 1; k <= n; k++) { split(edges[k], p, "\t"); print "G", p[1], p[2] }
  n = asortish(medges, men)
  for (k = 1; k <= n; k++) { split(medges[k], p, "\t"); print "R", p[1], p[2] }
  for (k = 1; k <= vn; k++) print vrec[k]
}

# The counts of one milestone: the two denominators, then one entry per declared status.
# total is every issue; required is the denominator the milestone status derivation uses, so
# no surface can print "n of total" for a milestone the engine calls DONE. Computed here and
# nowhere else, and computed again after the gate, which changes statuses underneath it.
function mcounts(id,   j, iid, tot, v, nsv, sv, cnt, out) {
  tot = 0
  for (j = 1; j <= inum; j++) {
    iid = iids[j]
    if (it[iid, "milestone"] != id) continue
    tot++; cnt[status[iid]]++
  }
  out = "total=" tot ",required=" (tot - (cnt["CANCELLED"] + 0))
  nsv = split(ISTATUS, sv, " ")
  for (v = 1; v <= nsv; v++) out = out "," sv[v] "=" (cnt[sv[v]] + 0)
  return out
}

# A record's prose is rendered into the site's plan pages, where a template delimiter left in
# the text reaches the built HTML unresolved and fails the site job (I3009: I3007 on 10-18,
# I3838 on 10-21). It is refused where it is written, named in words so that neither this file
# nor the refusal holds one: the delimiters are built from a single brace.
function delimiter(id, key, val,   b) {
  b = "{"
  if (index(val, b b))  finding("FAIL", "template_delimiter", id, key " holds a double brace, which the site renders raw on the record's plan page; describe the template syntax in words")
  if (index(val, b "%")) finding("FAIL", "template_delimiter", id, key " holds a brace-percent, which the site renders raw on the record's plan page; describe the template syntax in words")
  if (index(val, b "#")) finding("FAIL", "template_delimiter", id, key " holds a brace-hash, which the site renders raw on the record's plan page; describe the template syntax in words")
}

function finding(level, code, subject, message) {
  vrec[++vn] = "V" OFS level OFS code OFS subject OFS message
  if (level == "FAIL") vfail++
}

# does path a contain path b, or the reverse? Trailing slashes and ./ are normalised so
# that a hand-written scope entry cannot silently miss an overlap.
function overlap(a, b) {
  sub(/^\.\//, "", a); sub(/\/+$/, "", a)
  sub(/^\.\//, "", b); sub(/\/+$/, "", b)
  if (a == b) return 1
  if (index(b, a "/") == 1) return 1
  if (index(a, b "/") == 1) return 1
  return 0
}

# insertion sort: the edge list is small and awk 3.2-era has no asort()
function asortish(arr, n,   i, j, t) {
  for (i = 2; i <= n; i++) { t = arr[i]; j = i - 1
    while (j >= 1 && arr[j] > t) { arr[j + 1] = arr[j]; j-- }
    arr[j + 1] = t }
  return n
}
