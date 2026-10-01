# Recurring invoices

A customer can be billed on a schedule. The feature has two parts.

## Part 1 — schedules

A new module `ledgerlite/recurring.py` with:

- `Schedule(start: datetime.date, every: str, count: int)` — a dataclass. `every` is
  `"monthly"` or `"quarterly"`; anything else raises `ValueError`. `count` is the number
  of occurrences and must be at least 1 (`ValueError` otherwise).
- `Schedule.dates() -> list[datetime.date]` — the `count` issue dates, starting at `start`.
  A monthly schedule advances one calendar month, a quarterly one three. When the start
  day does not exist in a later month, the date is the last day of that month
  (a monthly schedule from 2026-01-31 gives 2026-01-31, 2026-02-28, 2026-03-31, ...):
  the day is always taken from `start`, never from the previous occurrence.

Tests for part 1 go in `tests/test_recurring.py`.

## Part 2 — generating invoices

- `recurring.generate(template: Invoice, schedule: Schedule, first_number: int)
  -> list[Invoice]` — one invoice per schedule date: a copy of the template's lines,
  customer and country, with `issued` set to the date and `number` set to
  `"R-<n>"` where `n` counts up from `first_number`. The template itself is not modified.
- A CLI command `ledgerlite recur FILE --template NUMBER --every monthly|quarterly
  --count N --first N` that loads FILE, generates from the invoice numbered NUMBER starting
  on that invoice's issue date, appends the generated invoices and saves FILE. It prints
  one line per generated invoice: `<number> <issued>`.

Tests for part 2 go in `tests/test_recurring.py` too.
