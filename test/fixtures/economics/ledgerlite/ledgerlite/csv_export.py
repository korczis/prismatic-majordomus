"""CSV export: one row per invoice, amounts as decimal strings in major units."""

from __future__ import annotations

import csv
import io

from . import dates
from .currency import currency
from .invoice import Invoice

HEADER = ["number", "issued", "customer", "currency", "net", "vat", "gross"]


def decimal(minor: int, code: str) -> str:
    """An amount in major units with a dot: 1050 EUR -> '10.50'."""
    digits = currency(code).minor_digits
    if digits == 0:
        return str(minor)
    whole, frac = divmod(minor, 10**digits)
    return f"{whole}.{str(frac).zfill(digits)}"


def export(invoices: list[Invoice]) -> str:
    out = io.StringIO()
    w = csv.writer(out, lineterminator="\n")
    w.writerow(HEADER)
    for inv in invoices:
        w.writerow(
            [
                inv.number,
                dates.render(inv.issued),
                inv.customer,
                inv.currency,
                decimal(inv.net().minor, inv.currency),
                decimal(inv.vat().minor, inv.currency),
                decimal(inv.gross().minor, inv.currency),
            ]
        )
    return out.getvalue()
