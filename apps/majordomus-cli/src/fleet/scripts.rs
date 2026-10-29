//! The programs a rollout runs on a machine, each one fixed here and handed its values as
//! positional arguments: nothing a declaration says is ever spliced into a program's text.
//!
//! Every program is POSIX `sh`, answers with `key=value` lines on stdout and exits 0 when it
//! did what it was asked, 10 when it declined and says why in a `refused=` line, and
//! anything else when it failed. They run as `sh -c <program> majordomus-fleet <args...>`,
//! on this machine directly and on another through `ssh`, so the same text runs in both.

/// What the machine is and runs. `$1` is a hub's checkout relative to the home directory
/// (empty for none), `$2` its port, `$3` the install directory relative to the home
/// directory.
pub const PROBE: &str = r#"
h="$HOME"
echo "home=$h"
echo "os=$(uname -s)"
echo "arch=$(uname -m)"
bin="$h/$3/majordomus"
if [ -x "$bin" ]; then
  v=$("$bin" --version 2>/dev/null | awk '{print $2}')
  echo "installed=$v"
fi
if [ -n "$1" ]; then
  d="$h/$1"
  if [ -e "$d/.git" ]; then
    echo "checkout=present"
    echo "branch=$(git -C "$d" symbolic-ref --short -q HEAD || echo '(detached)')"
    echo "head=$(git -C "$d" rev-parse HEAD)"
    if [ -n "$(git -C "$d" status --porcelain --untracked-files=no)" ]; then
      echo "dirty=yes"
    else
      echo "dirty=no"
    fi
  else
    echo "checkout=absent"
  fi
  r=$(curl -fsS -m 3 "http://127.0.0.1:$2/api/v1/ready" 2>/dev/null | tr -d '\n') || r=""
  echo "ready=$r"
fi
ps -A -o args= 2>/dev/null | grep -E '[m]ajordomus(-cli)? serve' | sed 's/^/server=/' || true
"#;

/// Install a release with the published installer. `$1` is the installer's URL, `$2` the
/// version. The installer verifies the archive's digest before it touches anything, keeps
/// a tree a running process still uses, and swaps the launchers in one rename each.
pub const INSTALL: &str = r#"
set -eu
out=$(curl -fsSL "$1" | sh -s -- --version "$2" 2>&1) || {
  printf '%s\n' "$out" | tail -n 5 | sed 's/^/log=/'
  exit 1
}
printf '%s\n' "$out" | tail -n 3 | sed 's/^/log=/'
"#;

/// Bring a hub's checkout to its remote's default branch, never rewriting work. `$1` is
/// the checkout relative to the home directory, `$2` the URL to clone when there is none.
///
/// A hub serves the trunk. Its checkout is either on the default branch, which is
/// fast-forwarded, or detached — the shape a linked worktree of a machine's own checkout
/// takes, since that checkout holds the branch — which is moved to the remote's head only
/// when that head contains it. A checkout with uncommitted changes, on another branch, or
/// holding commits the remote does not is declined and left exactly as it was.
pub const CHECKOUT: &str = r#"
set -eu
d="$HOME/$1"
if [ ! -e "$d/.git" ]; then
  mkdir -p "$(dirname "$d")"
  git clone --quiet "$2" "$d"
  echo "cloned=yes"
fi
cd "$d"
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
  echo "refused=the checkout has uncommitted changes; they are left as they are"
  exit 10
fi
git fetch --quiet origin
def=$(git symbolic-ref --short -q refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##') || def=""
[ -n "$def" ] || def=master
before=$(git rev-parse HEAD)
echo "before=$before"
if ! git merge-base --is-ancestor HEAD "origin/$def"; then
  echo "refused=the checkout holds commits origin/$def does not; they are left as they are"
  exit 10
fi
b=$(git symbolic-ref --short -q HEAD || true)
if [ -z "$b" ]; then
  git checkout --quiet --detach "origin/$def"
elif [ "$b" = "$def" ]; then
  git merge --quiet --ff-only "origin/$def" >/dev/null
else
  echo "refused=the checkout is on $b, not $def"
  exit 10
fi
echo "head=$(git rev-parse HEAD)"
"#;

/// Write and (re)start a hub's service. `$1` is `systemd` or `launchd`, `$2` the service's
/// file, `$3` its content, `$4` its name, `$5` the hub's checkout (absolute), `$6` the
/// launcher, `$7` `restart` or `keep`. With `keep` and a file that already holds this
/// content, nothing is touched.
///
/// A server some other starter left serving the checkout holds its lease, and the
/// service's would exit at once; `serve stop` asks that one to go first.
pub const SERVICE: &str = r#"
set -eu
mkdir -p "$(dirname "$2")"
if [ "$7" = keep ] && [ -f "$2" ] && [ "$(cat "$2")" = "$3" ]; then
  echo "action=unchanged"
  exit 0
fi
printf '%s\n' "$3" > "$2.tmp"
mv -f "$2.tmp" "$2"
case "$1" in
  systemd)
    u=$(dirname "$2")
    if [ -f "$u/majordomus.service" ] && grep -q 'scripts/mesh-hub' "$u/majordomus.service"; then
      systemctl --user disable --now majordomus.service >/dev/null 2>&1 || true
      rm -f "$u/majordomus.service"
      echo "replaced=majordomus.service"
    fi
    systemctl --user daemon-reload
    systemctl --user enable "$4" >/dev/null 2>&1
    "$6" serve stop --repo "$5" >/dev/null 2>&1 || true
    systemctl --user restart "$4"
    if loginctl enable-linger "$(id -un)" >/dev/null 2>&1; then
      echo "linger=yes"
    else
      echo "linger=unavailable"
    fi
    ;;
  launchd)
    uid=$(id -u)
    launchctl bootout "gui/$uid/$4" >/dev/null 2>&1 || true
    "$6" serve stop --repo "$5" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$uid" "$2"
    ;;
  *)
    echo "refused=no service manager $1"
    exit 10
    ;;
esac
echo "action=restarted"
"#;

/// Replace every server this machine runs from an installed tree of another version.
/// `$1` is the install prefix relative to the home directory, `$2` the version, `$3` the
/// launcher, `$4` the hub's checkout (absolute; empty for none), which its service owns.
///
/// After an upgrade a server keeps running the tree it was started from, and `serve
/// ensure` deliberately leaves it (the executable it names was not replaced, it was
/// superseded). Each one is asked to stop and started again through the launcher on the
/// port it had. A server built from a checkout is that checkout's, and is left alone; so is
/// one bound beyond loopback (`--host`), which some service or person started on purpose
/// and which `serve ensure` would bring back on loopback only.
pub const SERVERS: &str = r#"
pfx="$HOME/$1/versions/"
ps -A -o args= 2>/dev/null | grep -E '[m]ajordomus(-cli)? serve' | while IFS= read -r line; do
  case "$line" in "$pfx"*) ;; *) continue ;; esac
  v=${line#"$pfx"}
  v=${v%%/*}
  [ "$v" = "$2" ] && continue
  repo=$(printf '%s\n' "$line" | sed -n 's/.* --repo \([^ ]*\).*/\1/p')
  port=$(printf '%s\n' "$line" | sed -n 's/.* --port \([0-9][0-9]*\).*/\1/p')
  [ -n "$repo" ] || continue
  [ "$repo" = "$4" ] && continue
  case "$line" in
    *" --host "*)
      echo "log=$repo: $v left running; it is bound beyond loopback and its starter owns it"
      continue
      ;;
  esac
  "$3" serve stop --repo "$repo" >/dev/null 2>&1 || true
  if "$3" serve ensure --repo "$repo" ${port:+--port "$port"} >/dev/null 2>&1; then
    echo "log=$repo: $v → $2"
  else
    echo "log=$repo: $v stopped, and did not start again at $2"
    echo "failed=yes"
  fi
done
"#;

/// Wait for a hub to answer at a version, then say what its mesh sees. `$1` is the port,
/// `$2` the version, `$3` how many seconds to wait.
pub const VERIFY: &str = r#"
i=0
while :; do
  r=$(curl -fsS -m 3 "http://127.0.0.1:$1/api/v1/ready" 2>/dev/null | tr -d '\n') || r=""
  case "$r" in
    *"\"version\": \"$2\""*|*"\"version\":\"$2\""*) break ;;
  esac
  i=$((i + 2))
  if [ "$i" -ge "$3" ]; then
    echo "ready=$r"
    echo "refused=the hub did not answer at $2 within $3 s"
    exit 10
  fi
  sleep 2
done
echo "ready=$r"
echo "mesh=$(curl -fsS -m 5 "http://127.0.0.1:$1/api/v1/mesh" 2>/dev/null | tr -d '\n')"
echo "nodes=$(curl -fsS -m 5 "http://127.0.0.1:$1/api/v1/mesh/nodes" 2>/dev/null | tr -d '\n')"
"#;
