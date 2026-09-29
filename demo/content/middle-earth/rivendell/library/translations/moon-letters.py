"""Moon-letters: runes that only show under a moon of the same shape and
season as the night they were written."""
from dataclasses import dataclass
from datetime import date


@dataclass
class Moon:
    phase: str      # "crescent", "half", "full", ...
    season: str     # "midsummer", ...

    def matches(self, other: "Moon") -> bool:
        return self.phase == other.phase and self.season == other.season


def readable(written_under: Moon, tonight: Moon, day: date) -> bool:
    # Durin's Day adds a thrush and a setting sun; that's the keyhole, not the letters.
    return written_under.matches(tonight)


THROR_MAP = Moon("crescent", "midsummer")

if __name__ == "__main__":
    print(readable(THROR_MAP, Moon("crescent", "midsummer"), date(2941, 6, 21)))  # True
    print(readable(THROR_MAP, Moon("full", "winter"), date(2941, 12, 1)))        # False
