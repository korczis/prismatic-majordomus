"""Dates are UTC and serialised as ISO 8601 dates."""

from __future__ import annotations

import datetime


def parse(text: str) -> datetime.date:
    return datetime.date.fromisoformat(text)


def render(day: datetime.date) -> str:
    return day.isoformat()


def month_key(day: datetime.date) -> str:
    return f"{day.year:04d}-{day.month:02d}"


def today() -> datetime.date:
    return datetime.datetime.now(datetime.timezone.utc).date()
