"""An amount of money: integer minor units and an ISO 4217 currency code."""

from __future__ import annotations

from dataclasses import dataclass


class CurrencyMismatch(ValueError):
    """Two amounts in different currencies were combined."""


@dataclass(frozen=True, order=True)
class Money:
    minor: int
    currency: str = "EUR"

    def __post_init__(self) -> None:
        if not isinstance(self.minor, int) or isinstance(self.minor, bool):
            raise TypeError(f"Money.minor must be an int, got {type(self.minor).__name__}")
        if len(self.currency) != 3 or not self.currency.isupper():
            raise ValueError(f"not an ISO 4217 code: {self.currency!r}")

    def _same(self, other: "Money") -> None:
        if self.currency != other.currency:
            raise CurrencyMismatch(f"{self.currency} and {other.currency}")

    def __add__(self, other: "Money") -> "Money":
        self._same(other)
        return Money(self.minor + other.minor, self.currency)

    def __sub__(self, other: "Money") -> "Money":
        self._same(other)
        return Money(self.minor - other.minor, self.currency)

    def __neg__(self) -> "Money":
        return Money(-self.minor, self.currency)

    def times(self, quantity: int) -> "Money":
        return Money(self.minor * quantity, self.currency)

    @staticmethod
    def zero(currency: str = "EUR") -> "Money":
        return Money(0, currency)

    @staticmethod
    def total(amounts, currency: str = "EUR") -> "Money":
        out = Money.zero(currency)
        for a in amounts:
            out = out + a
        return out


def round_half_even(numerator: int, denominator: int) -> int:
    """numerator / denominator rounded to the nearest int, ties to even. Exact, no floats."""
    if denominator <= 0:
        raise ValueError("denominator must be positive")
    q, r = divmod(numerator, denominator)
    twice = 2 * r
    if twice > denominator or (twice == denominator and q % 2 == 1):
        q += 1
    return q


def round_half_up(numerator: int, denominator: int) -> int:
    """numerator / denominator rounded to the nearest int, ties away from zero."""
    if denominator <= 0:
        raise ValueError("denominator must be positive")
    sign = -1 if numerator < 0 else 1
    q, r = divmod(abs(numerator), denominator)
    if 2 * r >= denominator:
        q += 1
    return sign * q
