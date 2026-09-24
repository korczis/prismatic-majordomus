"""The `ledgerlite` command."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from . import csv_export, dates, report, storage
from .currency import format_amount
from .invoice import Invoice


def _load(path: str) -> list[Invoice]:
    p = Path(path)
    return storage.load(p) if p.exists() else []


def cmd_create(args) -> int:
    invoices = _load(args.file)
    if any(i.number == args.number for i in invoices):
        print(f"invoice {args.number} already exists", file=sys.stderr)
        return 1
    issued = dates.parse(args.issued) if args.issued else dates.today()
    invoices.append(Invoice(args.number, args.customer, args.country, issued))
    storage.save(Path(args.file), invoices)
    return 0


def _find(invoices, number):
    for inv in invoices:
        if inv.number == number:
            return inv
    raise SystemExit(f"no invoice {number}")


def cmd_add_line(args) -> int:
    invoices = _load(args.file)
    _find(invoices, args.number).add(args.description, args.quantity, args.unit_price)
    storage.save(Path(args.file), invoices)
    return 0


def cmd_total(args) -> int:
    inv = _find(_load(args.file), args.number)
    print(f"net   {format_amount(inv.net())}")
    print(f"vat   {format_amount(inv.vat())}")
    print(f"gross {format_amount(inv.gross())}")
    return 0


def cmd_report(args) -> int:
    for m in report.monthly_totals(_load(args.file)):
        print(f"{m.month} {m.invoices} {format_amount(m.gross)}")
    return 0


def cmd_export(args) -> int:
    sys.stdout.write(csv_export.export(_load(args.file)))
    return 0


def parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="ledgerlite")
    sub = p.add_subparsers(dest="command", required=True)

    c = sub.add_parser("create")
    c.add_argument("file")
    c.add_argument("number")
    c.add_argument("--customer", required=True)
    c.add_argument("--country", required=True)
    c.add_argument("--issued")
    c.set_defaults(func=cmd_create)

    a = sub.add_parser("add-line")
    a.add_argument("file")
    a.add_argument("number")
    a.add_argument("description")
    a.add_argument("quantity", type=int)
    a.add_argument("unit_price", type=int)
    a.set_defaults(func=cmd_add_line)

    t = sub.add_parser("total")
    t.add_argument("file")
    t.add_argument("number")
    t.set_defaults(func=cmd_total)

    r = sub.add_parser("report")
    r.add_argument("file")
    r.set_defaults(func=cmd_report)

    e = sub.add_parser("export")
    e.add_argument("file")
    e.set_defaults(func=cmd_export)
    return p


def main(argv=None) -> int:
    args = parser().parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
