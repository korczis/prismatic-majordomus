# Conventions

1. **Money is integer minor units.** Never a float. `Money(1050, "EUR")` is 10.50 EUR.
   See `decisions/0001-integer-minor-units.md`.
2. **VAT is computed and rounded per line**, half to even, never on the invoice total.
   See `decisions/0002-vat-rounding-per-line.md`.
3. **Percentages are basis points** (`int`): 2100 is 21 %.
4. **Dates are UTC** and serialised as ISO 8601 dates (`2026-01-31`).
5. **A public rename keeps a deprecated alias for one release.** The old name keeps
   working, calls the new one, and emits `DeprecationWarning` naming the new name.
6. **The storage format is versioned.** A file carries `"version"`; a reader accepts every
   earlier version and migrates it on load. See `decisions/0003-storage-format.md`.
7. **Every behaviour change comes with a test** under `tests/`.
