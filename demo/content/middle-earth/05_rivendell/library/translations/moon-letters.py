"""Moon-letters: runes that only show under a moon of the same shape and
season as the night they were written.

Provenance: a scribal tool of the Rivendell library, written to read the
moon-letters on Thrór's map (see `05_rivendell/library/maps/` and
`01_lonely-mountain/`). The dwarves wrote the message in silver pen under a
crescent moon of midsummer; it read: "Stand by the grey stone when the
thrush knocks, and the setting sun with the last light of Durin's Day will
shine upon the key-hole."
"""
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


# Thrór's map was written under a crescent moon of midsummer (TA 2770-ish),
# and read by Elrond under the same moon in 2941.
THROR_MAP = Moon("crescent", "midsummer")
MOON_OF_2941 = Moon("crescent", "midsummer")
A_WRONG_NIGHT = Moon("full", "winter")

if __name__ == "__main__":
    print(readable(THROR_MAP, MOON_OF_2941, date(2941, 6, 21)))   # True
    print(readable(THROR_MAP, A_WRONG_NIGHT, date(2941, 12, 1)))  # False
