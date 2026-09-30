"""tides.py: high water at the headland of Dol Amroth.

Filed by the harbour-master of Tirith Aear. The Bay of Belfalas has two
tides a day, about twelve hours and twenty-five minutes apart, which is the
only thing in Gondor that keeps better time than the Steward.
Cross-refs: README.md, ../umbar/requisition-3019.log (who else uses the tide).
"""

TIDE_PERIOD_H = 12.42  # hours between high waters
FIRST_HIGH_H = 5.5     # high water after midnight on the reckoning day


def high_waters(day: int, first=FIRST_HIGH_H):
    """Times (hours past midnight) of high water on `day` days after the reckoning."""
    t = (first + day * (2 * TIDE_PERIOD_H - 24)) % TIDE_PERIOD_H
    out = []
    while t < 24:
        out.append(t)
        t += TIDE_PERIOD_H
    return out


def fmt(h: float) -> str:
    hh, mm = divmod(round(h * 60), 60)
    return f"{hh:02d}:{mm:02d}"


if __name__ == "__main__":
    # a week of tides from the day the Swan Knights rode north (3019-03-06)
    for d in range(7):
        print(f"3019-03-{6 + d:02d}", "  ".join(fmt(h) for h in high_waters(d)))
    print("gulls: always")
