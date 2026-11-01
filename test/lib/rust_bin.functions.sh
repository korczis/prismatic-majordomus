# shellcheck shell=bash
# shellcheck disable=SC2034 # the globals are declared for the reader, never read here
# What lib/rust_bin.sh defines, for shellcheck alone: one stub per function and one empty
# assignment per global, in rust_bin.sh's order. Nothing sources this file at run time;
# test/lib.sh points its two `. "$ROOT/lib/rust_bin.sh"` lines here with `# shellcheck
# source=` (I4381).
#
# Why a stub and not the file. Every case sources test/lib.sh, and shellcheck -x followed it
# into rust_bin.sh's 1942 lines once per case: shell-lint took 1388 s of the structure job at
# p90, and following it linted none of rust_bin.sh (shellcheck reports no finding inside a
# file it follows; rust_bin.sh is linted on its own, in shell-lint's lib/ set). Pointing the
# directive at /dev/null instead lost what shellcheck knew from the names: that
# mj_cargo_target_dir is a shell function, so `xargs mj_cargo_target_dir`, `sudo
# mj_rust_resolve` and `find -exec mj_cargo_target_dir` were no longer SC2033. This file
# keeps the names, one line each.
#
# It is checked, not trusted: test/cases/4381_the_structure_job_fits_in_half_its_bound.sh
# reads rust_bin.sh's top-level function names and global assignments and fails on any
# difference from this file, in either direction or in order. To rewrite it after a change
# to rust_bin.sh, keep this header and replace the lines below it with the output of
#   sed -n -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)() .*/\1() { :; }/p' \
#     -e 's/^\([A-Za-z_][A-Za-z0-9_]*\)=.*/\1=/p' lib/rust_bin.sh
mj_cargo_target_dir() { :; }
mj_rust_bin() { :; }
mj_rust_is_launcher() { :; }
mj_rust_stale() { :; }
mj_rust_installed() { :; }
MJ_RUST_CRATE_INPUTS=
mj_rust_built_commit() { :; }
mj_rust_same_generation() { :; }
mj_rust_resolve() { :; }
mj_rust_share() { :; }
mj_rust_target_named_by() { :; }
mj_rust_physical() { :; }
mj_rust_owns() { :; }
mj_rust_outside() { :; }
mj_rust_clean() { :; }
mj_rust_bin_missing() { :; }
MJ_MIN_FREE_MB_DEFAULT=
MJ_RECLAIM_BELOW_MB_DEFAULT=
MJ_RECLAIM_EVERY_S=
mj_rust_reclaim() { :; }
mj_free_mb() { :; }
mj_rust_space_check() { :; }
MJ_INCREMENTAL_IDLE_MIN_DEFAULT=
mj_cargo_metadata_dir() { :; }
mj_cargo_build_dir() { :; }
mj_rust_is_build_dir() { :; }
mj_rust_mtime() { :; }
mj_rust_kb() { :; }
mj_rust_human_kb() { :; }
mj_rust_prune_incremental_of() { :; }
MJ_DEPS_IDLE_MIN_DEFAULT=
mj_rust_profile_dirs() { :; }
mj_rust_profile_users() { :; }
mj_rust_locked() { :; }
mj_rust_crate_stems() { :; }
mj_rust_generation() { :; }
mj_rust_open_under() { :; }
mj_rust_prune_deps_of() { :; }
mj_rust_sweep_profile() { :; }
mj_rust_prune_build_dir() { :; }
MJ_SHARED_BUILD_CAP_MB_DEFAULT=
MJ_EVICT_IDLE_MIN_DEFAULT=
MJ_EVICT_PRESSURE_IDLE_MIN_DEFAULT=
MJ_BOUND_SWEEP_EVERY_S=
MJ_EVICT_LOCK_WAIT_DEFAULT=
mj_rust_bound_state() { :; }
mj_rust_pressure_idle() { :; }
mj_rust_bound_due() { :; }
mj_rust_named_versions() { :; }
mj_rust_deps_generations() { :; }
mj_rust_bound_plan() { :; }
mj_rust_evict_in_profile() { :; }
mj_rust_bound_build_dir() { :; }
mj_rust_bound_after_build() { :; }
MJ_PRIVATE_BUILD_IDLE_MIN_DEFAULT=
MJ_PRIVATE_SWEEP_EVERY_S=
mj_rust_private_unheld() { :; }
mj_rust_reclaim_private() { :; }
mj_rust_prune_incremental() { :; }
mj_rust_largest_build_dirs() { :; }
MJ_BUILD_LOCK_WAIT_DEFAULT=
MJ_BUILD_LOCK_WAIT_LOW_DISK_DEFAULT=
mj_cargo_private_dir() { :; }
mj_build_cache_start() { :; }
mj_cargo_bounded() { :; }
mj_rust_reasoning_report() { :; }
mj_rust_monitor_brief() { :; }
mj_rust_prs_brief() { :; }
