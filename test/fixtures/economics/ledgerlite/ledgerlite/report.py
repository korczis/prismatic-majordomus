"""Monthly summaries across invoices."""

from __future__ import annotations

from dataclasses import dataclass

from . import dates
from .invoice import Invoice
from .money import Money


@dataclass(frozen=True)
class MonthTotals:
    month: str
    invoices: int
    net: Money
    vat: Money
    gross: Money


def monthly_totals(invoices: list[Invoice], currency: str = "EUR") -> list[MonthTotals]:
    """Totals per calendar month, oldest first. Invoices in other currencies are skipped."""
    buckets: dict[str, list[Invoice]] = {}
    for inv in invoices:
        if inv.currency != currency:
            continue
        buckets.setdefault(dates.month_key(inv.issued), []).append(inv)
    out = []
    for month in sorted(buckets):
        group = buckets[month]
        net = Money.total((i.net() for i in group), currency)
        vat = Money.total((i.vat() for i in group), currency)
        out.append(MonthTotals(month, len(group), net, vat, net + vat))
    return out
