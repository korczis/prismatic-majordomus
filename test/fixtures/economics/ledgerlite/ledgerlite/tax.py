"""VAT rates by country and the VAT of an invoice line (decision 0002)."""

from __future__ import annotations

from .money import round_half_even

# Standard rates in basis points.
VAT_RATES = {
    "AT": 2000,
    "CZ": 2100,
    "DE": 1900,
    "FR": 2000,
    "NL": 2100,
    "SK": 2300,
}


def rate_for(country: str) -> int:
    try:
        return VAT_RATES[country]
    except KeyError:
        raise ValueError(f"no VAT rate for country {country!r}") from None


def line_tax(net_minor: int, rate_bp: int) -> int:
    """The VAT of one line, in minor units, rounded half to even (decision 0002)."""
    return round_half_even(net_minor * rate_bp, 10_000)
