# majordomus-covers: none
# A display that is watched and not touched must not be put to sleep (the kiosk, and any
# display in full screen): share/ui/observatory.js holds a screen wake lock the way a video
# does. The browser releases a lock whenever the page is hidden, so the script must ask
# again when it is visible, and must let go when the display is disposed. This reads the
# script a page is served and proves the three obligations are written down; that a given
# browser grants the lock is the browser's and is not claimed here.
. "$ROOT/test/lib.sh"
JS="$ROOT/share/ui/observatory.js"
[ -f "$JS" ] || { echo "    no $JS"; exit 1; }

grep -q "navigator.wakeLock" "$JS" || { echo "    the display never asks for a screen wake lock"; exit 1; }
grep -q "wakeLock.request('screen')" "$JS" || { echo "    the lock asked for is not the screen's"; exit 1; }

# re-acquired on visibility: the visibilitychange handler calls the holder
awk '/const onVisible = /,/addEventListener\(.visibilitychange/' "$JS" | grep -q "holdAwake()" \
  || { echo "    the lock is not asked for again when the page becomes visible"; exit 1; }

# released on dispose, and when full screen ends
awk '/dispose\(\) \{/,/^      \},/' "$JS" | grep -q "releaseAwake()" \
  || { echo "    dispose leaves the lock held"; exit 1; }
grep -q "addEventListener('fullscreenchange', onFullscreen)" "$JS" \
  || { echo "    leaving full screen does not release the lock"; exit 1; }

# one request at a time, and a grant that arrives unwanted or after dispose is given back
grep -q "wakePending" "$JS" || { echo "    a second request can leak a second lock"; exit 1; }
awk '/async function holdAwake\(\)/,/^    }$/' "$JS" | grep -qF "if (gone || !wantWake) { lock.release()" \
  || { echo "    a lock granted after dispose or after full screen ended is kept"; exit 1; }
awk '/dispose\(\) \{/,/^      \},/' "$JS" | grep -q "gone = true" \
  || { echo "    dispose does not mark the display gone"; exit 1; }

# an absent or refusing API is said, never silent
grep -q "dataset.awake" "$JS" || { echo "    the display does not say whether it holds the lock"; exit 1; }
grep -q "'unsupported'" "$JS" && grep -q "'refused'" "$JS" \
  || { echo "    an unsupported or refused lock is not named"; exit 1; }
echo "    the display holds, renews and releases a screen wake lock, and says when it cannot"
