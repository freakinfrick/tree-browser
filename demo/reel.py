#!/usr/bin/env python3
"""The README GIF: a highlight reel cut from the full take.

Each cut is (caption prefix, offset, length): it starts `offset` seconds after
that caption first appears in caps.jsonl, so re-recording with new timing keeps
the cuts on their features. Usage: reel.py FULL.mp4 OUT.gif"""
import json, os, subprocess, sys

S = os.path.dirname(os.path.abspath(__file__))
CUTS = [
    ("color =", 0.0, 2.2),            # heat colors and the legend
    ("l opens", 1.7, 2.6),            # PDF page flip
    ("e explodes", 0.0, 2.2),
    ("audio", 2.0, 2.4),              # waveform, scrubber moving
    ("images and GIFs", 0.3, 2.0),
    ("live:", 0.2, 2.8),              # Moria re-heats, the ripple climbs
    ("RFC 0001", 0.2, 2.2),           # Fangorn's rename RFC through glow (its red shows in the first cut)
    ("the mouse wheel", 1.0, 2.0),
    ("markdown through glow", 1.5, 2.6),   # glow, then d: the staged diff
    ("code through bat", 0.8, 2.6),
    ("s opens a shell", 0.3, 3.0),
    ("/ finds", 0.8, 2.4),
    ("Mordor burns", 0.3, 1.6),
    ("o sorts", 0.5, 3.0),
    (", settings", 1.3, 3.0),
    ("? lists", 0.1, 1.6),
]
FPS, WIDTH = 15, 960


def main(full, out):
    starts = {}
    for line in open(os.path.join(S, "caps.jsonl")):
        c = json.loads(line)
        starts.setdefault(c["cap"], c["t"])
    parts, labels = [], []
    for i, (prefix, off, length) in enumerate(CUTS):
        t = next((t for cap, t in starts.items() if cap.startswith(prefix)), None)
        if t is None:
            raise SystemExit(f"no caption starts with {prefix!r}")
        parts.append(f"[0:v]trim=start={t + off:.2f}:duration={length},setpts=PTS-STARTPTS[v{i}]")
        labels.append(f"[v{i}]")
    graph = ";".join(parts) + f";{''.join(labels)}concat=n={len(CUTS)}:v=1:a=0," \
        f"fps={FPS},scale={WIDTH}:-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];" \
        "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", full, "-filter_complex", graph, out], check=True)
    total = sum(c[2] for c in CUTS)
    print(f"{out}: {len(CUTS)} cuts, {total:.1f} s, {os.path.getsize(out) / 1e6:.1f} MB")


if __name__ == "__main__":
    main(*sys.argv[1:3])
