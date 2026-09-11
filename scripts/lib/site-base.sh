#!/usr/bin/env bash
# site-base.sh — where the published site lives, read once.
#
# `site/config.toml` states the site's origin, and two gates need to take it apart:
# scripts/site-check, to know which paths in the build are the site's own, and
# scripts/site-probe, to mount the build at the path the origin implies. Both derived it with
# the same line, byte for byte:
#
#   sed -n 's/^base_url = "\(.*\)"/\1/p' site/config.toml | sed -E 's#^[a-z]+://[^/]+##; s#/$##'
#
# That duplication cost nothing until the site moved to an apex domain. The old origin,
# https://korczis.github.io/prismatic-majordomus, carried a path; https://majordomus.dev does
# not — so both copies began deriving the empty string, on the same day, in the same commit.
# The usual argument against duplication is drift, and drift is not what happened: the two
# lines were still identical when they broke. What was lost was independence. Two gates a
# reviewer counts as two pieces of evidence were one witness wearing two hats, and one change
# of shape took both out at once. A repeated semantic definition across projections is a
# design defect here (ADR 0004); this is that defect with a demonstration attached.
#
# So: one reader, and it says what it found rather than only what it computed.
#
#   mj_site_base <config.toml>
#     MJ_SITE_BASE      the origin as declared, no trailing slash   https://majordomus.dev
#     MJ_SITE_PREFIX    the path part, possibly empty               ''  (or /prismatic-majordomus)
#     MJ_SITE_HOST      scheme and host                             https://majordomus.dev
#     MJ_SITE_MOUNTED   1 when the site is published under a path, 0 at an apex origin
#
# MJ_SITE_MOUNTED is the field that matters. An empty prefix is not a missing value to guard
# against with `[ -n "$PREFIX" ]` — it is a fact about the deployment, and the two cases want
# different assertions rather than one assertion and one silence.

mj_site_base() {
  local config="${1:?mj_site_base needs the path to config.toml}"
  [ -f "$config" ] || { echo "site-base: $config is missing" >&2; return 12; }

  MJ_SITE_BASE="$(sed -n 's/^base_url = "\(.*\)"/\1/p' "$config" | head -1 | sed 's#/$##')"
  [ -n "$MJ_SITE_BASE" ] || { echo "site-base: $config declares no base_url" >&2; return 12; }

  case "$MJ_SITE_BASE" in
    https://*|http://*) ;;
    *) echo "site-base: base_url is '$MJ_SITE_BASE', which states no origin" >&2; return 12 ;;
  esac

  MJ_SITE_PREFIX="$(printf '%s' "$MJ_SITE_BASE" | sed -E 's#^[a-z]+://[^/]+##')"
  MJ_SITE_HOST="${MJ_SITE_BASE%"$MJ_SITE_PREFIX"}"
  if [ -n "$MJ_SITE_PREFIX" ]; then MJ_SITE_MOUNTED=1; else MJ_SITE_MOUNTED=0; fi

  export MJ_SITE_BASE MJ_SITE_PREFIX MJ_SITE_HOST MJ_SITE_MOUNTED
}
