"""Invoices and their lines."""

from __future__ import annotations

import datetime
from dataclasses import dataclass, field

from . import tax
from .money import Money, round_half_up


@dataclass
class Line:
    description: str
    quantity: int
    unit_price: Money

    def net(self) -> Money:
        return self.unit_price.times(self.quantity)


@dataclass
class Invoice:
    number: str
    customer: str
    country: str
    issued: datetime.date
    currency: str = "EUR"
    lines: list[Line] = field(default_factory=list)

    def add(self, description: str, quantity: int, unit_price_minor: int) -> Line:
        line = Line(description, quantity, Money(unit_price_minor, self.currency))
        self.lines.append(line)
        return line

    def net(self) -> Money:
        return Money.total((l.net() for l in self.lines), self.currency)

    def vat(self) -> Money:
        rate = tax.rate_for(self.country)
        return Money(round_half_up(self.net().minor * rate, 10_000), self.currency)

    def gross(self) -> Money:
        return self.net() + self.vat()
