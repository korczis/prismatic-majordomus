# 0003 The storage format is versioned

An invoice file is a JSON object with `"version"` and `"invoices"`. Version 1 is the
current format (see `storage.py`). A new version never breaks old files: the reader
accepts each earlier version and migrates it in memory on load; the writer always writes
the newest version.
