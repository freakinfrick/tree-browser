#!/usr/bin/env python3
"""caps.jsonl -> 1080p PNG frames (deduped) -> concat list with real durations."""
import json, os, re, sys, time
from multiprocessing import Pool
from PIL import Image, ImageDraw, ImageFont

S = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(S, "frames")
CW, CH, COLS, ROWS, BAND = 20, 40, 96, 25, 80
FR = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
FB = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"
SGR = re.compile(r"(\x1b\[[0-9;]*m)")
DEF_FG, DEF_BG = (229, 229, 229), (9, 10, 15)

def c256(n):
    base = [(0,0,0),(205,0,0),(0,205,0),(205,205,0),(0,0,238),(205,0,205),(0,205,205),(229,229,229),
            (127,127,127),(255,0,0),(0,255,0),(255,255,0),(92,92,255),(255,0,255),(0,255,255),(255,255,255)]
    if n < 16: return base[n]
    if n < 232:
        n -= 16; v = [0, 95, 135, 175, 215, 255]; return (v[n // 36], v[(n // 6) % 6], v[n % 6])
    g = 8 + (n - 232) * 10; return (g, g, g)

def render(job):
    idx, ansi, cap = job
    R, B = ImageFont.truetype(FR, 33), ImageFont.truetype(FB, 33)
    CF = ImageFont.truetype(FB, 34)
    img = Image.new("RGB", (COLS * CW, ROWS * CH + BAND), DEF_BG)
    d = ImageDraw.Draw(img)
    for y, line in enumerate(ansi.split("\n")[:ROWS]):
        fg, bg, bold, x = DEF_FG, DEF_BG, False, 0
        for tok in SGR.split(line):
            if tok.startswith("\x1b["):
                ps = [int(p) if p else 0 for p in tok[2:-1].split(";")]; i = 0
                while i < len(ps):
                    p = ps[i]
                    if p == 0: fg, bg, bold = DEF_FG, DEF_BG, False
                    elif p == 1: bold = True
                    elif p == 22: bold = False
                    elif p in (38, 48) and i + 1 < len(ps) and ps[i + 1] == 2:
                        col = tuple(ps[i + 2:i + 5]); i += 4
                        if p == 38: fg = col
                        else: bg = col
                    elif p in (38, 48) and i + 1 < len(ps) and ps[i + 1] == 5:
                        col = c256(ps[i + 2]); i += 2
                        if p == 38: fg = col
                        else: bg = col
                    elif p == 39: fg = DEF_FG
                    elif p == 49: bg = DEF_BG
                    elif 30 <= p <= 37: fg = c256(p - 30)
                    elif 90 <= p <= 97: fg = c256(p - 82)
                    elif 40 <= p <= 47: bg = c256(p - 40)
                    i += 1
                continue
            for ch in tok:
                if x >= COLS: break
                if bg != DEF_BG:
                    d.rectangle([x * CW, y * CH, x * CW + CW - 1, y * CH + CH - 1], fill=bg)
                if ch != " ":
                    d.text((x * CW, y * CH + 2), ch, font=B if bold else R, fill=fg)
                x += 1
    # Caption band.
    top = ROWS * CH
    d.rectangle([0, top, COLS * CW, top + BAND], fill=(14, 16, 26))
    d.line([0, top, COLS * CW, top], fill=(68, 86, 168), width=2)
    if cap:
        w = d.textlength(cap, font=CF)
        d.text(((COLS * CW - w) / 2, top + (BAND - 40) / 2), cap, font=CF, fill=(225, 230, 250))
    path = os.path.join(OUT, f"f{idx:05d}.png")
    img.save(path, compress_level=1)
    return path

if __name__ == "__main__":
    caps = [json.loads(l) for l in open(os.path.join(S, "caps.jsonl"))]
    uniq = []
    for c in caps:
        if not uniq or (uniq[-1]["ansi"], uniq[-1]["cap"]) != (c["ansi"], c["cap"]):
            uniq.append(c)
    os.makedirs(OUT, exist_ok=True)
    for f in os.listdir(OUT):
        os.remove(os.path.join(OUT, f))
    jobs = [(i, c["ansi"], c["cap"]) for i, c in enumerate(uniq)]
    n = len(jobs); t0 = time.time(); paths = [None] * n
    print(f"{len(caps)} caps -> {n} unique frames", flush=True)
    with Pool(16) as pool:
        for k, p in enumerate(pool.imap(render, jobs, chunksize=4)):
            paths[k] = p
            if (k + 1) % 100 == 0 or k + 1 == n:
                el = time.time() - t0
                print(f"{k+1}/{n} rendered, ETA {el / (k + 1) * (n - k - 1):.0f}s", flush=True)
    with open(os.path.join(S, "list.txt"), "w") as f:
        for i, c in enumerate(uniq):
            dur = (uniq[i + 1]["t"] if i + 1 < n else c["t"] + 1.0) - c["t"]
            f.write(f"file '{paths[i]}'\nduration {dur:.4f}\n")
        f.write(f"file '{paths[-1]}'\n")
    print("list.txt written")
