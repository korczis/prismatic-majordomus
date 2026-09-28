# ledgerlite

A small invoicing library: money in integer minor units, VAT per line, JSON storage,
monthly reports and a CSV export, with a command-line front end.

## Layout

| path | what it holds |
|---|---|
| `ledgerlite/money.py` | `Money`: an amount in minor units and a currency code |
| `ledgerlite/currency.py` | the currency table and amount formatting |
| `ledgerlite/tax.py` | VAT rates by country and the tax of an invoice line |
| `ledgerlite/invoice.py` | `Invoice` and `Line`, totals |
| `ledgerlite/storage.py` | reading and writing invoices as JSON |
| `ledgerlite/report.py` | monthly summaries across invoices |
| `ledgerlite/csv_export.py` | CSV export of invoices |
| `ledgerlite/dates.py` | UTC date handling |
| `ledgerlite/cli.py` | the `ledgerlite` command |
| `docs/CONVENTIONS.md` | the conventions every change follows |
| `docs/decisions/` | the decisions behind the conventions |
| `docs/specs/` | specifications of planned features |

## Development

Python 3.10 or newer, standard library only.

```sh
python3 -m unittest discover -s tests -q
```
