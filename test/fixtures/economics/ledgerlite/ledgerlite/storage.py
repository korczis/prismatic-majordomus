"""Reading and writing invoices as JSON (decision 0003). The current format is version 1."""

from __future__ import annotations

import json
from pathlib import Path

from . import dates
from .invoice import Invoice
from .money import Money

VERSION = 1


def _invoice_to_dict(inv: Invoice) -> dict:
    return {
        "number": inv.number,
        "customer": inv.customer,
        "country": inv.country,
        "issued": dates.render(inv.issued),
        "lines": [
            {"description": l.description, "quantity": l.quantity, "unit_price": l.unit_price.minor}
            for l in inv.lines
        ],
    }


def _invoice_from_dict(d: dict) -> Invoice:
    inv = Invoice(d["number"], d["customer"], d["country"], dates.parse(d["issued"]))
    for l in d["lines"]:
        inv.add(l["description"], l["quantity"], l["unit_price"])
    return inv


def dumps(invoices: list[Invoice]) -> str:
    return json.dumps(
        {"version": VERSION, "invoices": [_invoice_to_dict(i) for i in invoices]},
        indent=2,
        sort_keys=True,
    )


def loads(text: str) -> list[Invoice]:
    doc = json.loads(text)
    version = doc.get("version")
    if version != VERSION:
        raise ValueError(f"unsupported storage version: {version!r}")
    return [_invoice_from_dict(d) for d in doc["invoices"]]


def load(path: Path) -> list[Invoice]:
    return loads(Path(path).read_text(encoding="utf-8"))


def save(path: Path, invoices: list[Invoice]) -> None:
    Path(path).write_text(dumps(invoices) + "\n", encoding="utf-8")
