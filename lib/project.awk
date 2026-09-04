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
#
# output (tab separated, first field is the record type):
#   P  name  repository  default_branch  active_milestone
#   M  id  status  order  priority  title  slug  total  done  ready  blocked  active  verify  cancelled
#   I  id  milestone  status  wave  priority  profile  parallel_safe  title  slug \
#      deps  blocked_by  dependents  scope  evidence_have  evidence_need
#   W  wave  id...                      one record per wave, ids space separated
#   G  from  to                          one record per dependency edge, in sorted order
#   V  level  code  subject  message     validation findings
#
# Statuses. Issue: BLOCKED READY ACTIVE VERIFY DONE CANCELLED. Milestone: PLANNED
# BLOCKED ACTIVE VERIFY DONE. Both are derived here and stored nowhere.

BEGIN { FS = "\t"; OFS = "\t" }

function clean(s) { gsub(/\t/, " ", s); return s }

# ---------------------------------------------------------------- ingest
$1 == "P" { pkey[$2] = $3; next }

$1 == "M" {
  id = $2; key = $3; val = $4
  if (!(id in mseen)) { mseen[id] = 1; mids[++mn] = id }
  m[id, key] = val
  if (key ~ /^evidence\.[0-9]+\.covers$/) { mev[id, val] = 1 }
  if (key ~ /^evidence_required\.[0-9]+$/) { mneed[id, ++mneedn[id]] = val }
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
    done[id] = (!cancelled[id] && it[id, "completed_at"] != "" && covered[id])
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
    if (it[id, "completed_at"] != "" && !covered[id] && !cancelled[id]) {
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
    else if (it[id, "completed_at"] != "" || it[id, "verified_at"] != "") st = "VERIFY"
    else if (it[id, "started_at"] != "")     st = "ACTIVE"
    else if (bb != "")                       st = "BLOCKED"
    else                                     st = "READY"
    status[id] = st
    if (bb != "" && (st == "DONE" || st == "ACTIVE" || st == "VERIFY"))
      finding("FAIL", "premature_execution", id, "is " st " while " bb " is not DONE")
    if (!left[id] && wave[id] == "" && st != "CANCELLED") wave[id] = 0
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

  # --- milestone status
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    tot = 0; nd = 0; nr = 0; nb = 0; na = 0; nv = 0; nc = 0
    for (j = 1; j <= inum; j++) {
      iid = iids[j]
      if (it[iid, "milestone"] != id) continue
      tot++
      s = status[iid]
      if (s == "DONE") nd++; else if (s == "READY") nr++; else if (s == "BLOCKED") nb++
      else if (s == "ACTIVE") na++; else if (s == "VERIFY") nv++; else if (s == "CANCELLED") nc++
    }
    mcov = 1
    for (k = 1; k <= mneedn[id]; k++) if (!((id, mneed[id, k]) in mev)) mcov = 0
    req = tot - nc
    if (m[id, "cancelled"] == "true")                       ms = "CANCELLED"
    else if (tot == 0)                                      ms = "PLANNED"
    else if (req > 0 && nd == req && mcov)                  ms = "DONE"
    else if (req > 0 && nd == req)                          ms = "VERIFY"
    else if (na > 0 || nv > 0 || nd > 0)                    ms = "ACTIVE"
    else if (nr == 0)                                       ms = "BLOCKED"
    else                                                    ms = "PLANNED"
    mstatus[id] = ms
    mrow[id] = tot "\t" nd "\t" nr "\t" nb "\t" na "\t" nv "\t" nc
    if (tot == 0) finding("WARN", "empty_milestone", id, "has no issues; it is an outcome nobody is executing")
    if (req > 0 && nd == req && !mcov) {
      miss = ""
      for (k = 1; k <= mneedn[id]; k++) if (!((id, mneed[id, k]) in mev)) miss = miss (miss == "" ? "" : ",") mneed[id, k]
      finding("WARN", "milestone_evidence_missing", id, "every issue is DONE but milestone evidence is missing for " miss "; status stays VERIFY")
    }
  }

  # --- the active milestone: lowest order that is ACTIVE, else lowest order not finished.
  #     Derived, so it can never contradict the issues underneath it.
  best = ""; bestord = ""
  for (pass = 1; pass <= 2; pass++) {
    for (i = 1; i <= mn; i++) {
      id = mids[i]
      if (mstatus[id] == "DONE" || mstatus[id] == "CANCELLED") continue
      if (pass == 1 && mstatus[id] != "ACTIVE") continue
      o = m[id, "order"] + 0
      if (best == "" || o < bestord) { best = id; bestord = o }
    }
    if (best != "") break
  }

  # ---------------------------------------------------------------- emit
  print "P", clean(pkey["name"]), clean(pkey["repository"]), clean(pkey["default_branch"]), best
  for (i = 1; i <= mn; i++) {
    id = mids[i]
    print "M", id, mstatus[id], m[id, "order"], m[id, "priority"], clean(m[id, "title"]), m[id, "slug"], mrow[id]
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
  for (k = 1; k <= vn; k++) print vrec[k]
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
