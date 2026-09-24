"""The currency table and the formatting of amounts for display."""

from __future__ import annotations

from dataclasses import dataclass

from .money import Money


@dataclass(frozen=True)
class Currency:
    code: str
    symbol: str
    minor_digits: int
    decimal_separator: str
    group_separator: str
    symbol_first: bool


CURRENCIES = {
    "EUR": Currency("EUR", "€", 2, ",", ".", False),
    "USD": Currency("USD", "$", 2, ".", ",", True),
    "GBP": Currency("GBP", "£", 2, ".", ",", True),
    "JPY": Currency("JPY", "¥", 0, ".", ",", True),
}


def currency(code: str) -> Currency:
    try:
        return CURRENCIES[code]
    except KeyError:
        raise ValueError(f"unknown currency: {code}") from None


def _group(digits: str, separator: str) -> str:
    parts = []
    while len(digits) > 3:
        parts.insert(0, digits[-3:])
        digits = digits[:-3]
    parts.insert(0, digits)
    return separator.join(parts)


def format_amount(amount: Money) -> str:
    """Format for display: `1.234,50 €`, `$1,234.50`, `¥1,235`."""
    c = currency(amount.currency)
    sign = "-" if amount.minor < 0 else ""
    minor = abs(amount.minor)
    if c.minor_digits:
        whole, frac = divmod(minor, 10 ** c.minor_digits)
        number = _group(str(whole), c.group_separator) + c.decimal_separator + str(frac).zfill(
            c.minor_digits
        )
    else:
        number = _group(str(minor), c.group_separator)
    if c.symbol_first:
        return f"{sign}{c.symbol}{number}"
    return f"{sign}{number} {c.symbol}"
