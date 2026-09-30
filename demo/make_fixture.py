#!/usr/bin/env python3
"""Build demo/demo/middle-earth: the tb demo tree, with staggered mtimes.

Text comes from content/ (committed). Images come from assets/ (run
fetch_images.py once; missing images are skipped). Generated here:
  *.pdf.src -> *.pdf    (PostScript -> ps2pdf, pages split on form feed)
  maps/atlas.pdf        (map images, one per page, via Ghostscript viewjpeg)
  drums-in-the-deep.wav, durins-bridge.dwg (binary, for the `file` fallback)
Folder mtimes are forced old, so any heat a folder shows comes from inside."""
import json, math, os, shutil, struct, subprocess, sys, tempfile, time, unicodedata, wave, zlib

S = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(S, "demo")
CONTENT = os.path.join(S, "content")
ASSETS = os.path.join(S, "assets")
ME = "middle-earth"
now, M, H, D = time.time(), 60, 3600, 86400

# The heat story: a few hot spots, everything else old. Unlisted files get a
# stable pseudo-random age of 1 month to 5 years.
AGES = {
    "README.md": 2 * H,
    "08_lothlorien/mirror-of-galadriel/visions.log": 25 * M,
    "08_lothlorien/mirror-of-galadriel/not-yet-come-to-pass.md": 40 * M,
    "11_rohan/helms-deep/wall-repairs.rs": 5 * H,
    "11_rohan/helms-deep/culvert-bug.md": 4 * H,
    "11_rohan/helms-deep/siege-postmortem.md": 3 * H,
    "15_mordor/mount-doom/.eye-is-watching.log": 90,
    "15_mordor/mount-doom/one-does-not-simply.txt": 2 * D,
    "15_mordor/barad-dur/orc-hr/shift-roster.csv": 1 * D,
    "15_mordor/barad-dur/orc-hr/complaints-to-sauron.txt": 7 * H,
    "15_mordor/barad-dur/eye-uptime.prom": 12 * M,
    "src/main.rs": 20 * M, "src/layout.rs": 45 * M, "src/anim.rs": 10 * M, "src/Cargo.toml": 3 * H,
    "10_fangorn/entmoot/minutes-day-2.txt": 6 * D,
    "05_rivendell/council-minutes/who-carries-the-ring.md": 6 * D,
    "02_the-shire/green-dragon-inn/tab.csv": 3 * D,
    "14_gondor/minas-tirith/beacon-schedule.toml": 8 * D,
}


def age(rel):
    if rel in AGES:
        return AGES[rel]
    return 30 * D + zlib.crc32(rel.encode()) % (1800 * D)


def ascii_(s):
    return unicodedata.normalize("NFKD", s).encode("ascii", "ignore").decode()


def text_pdf(src, dest):
    """Monospace pages: first line of each page bold, page number in the footer."""
    esc = lambda s: ascii_(s).replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    ps = ["%!PS", "/body /Courier findfont 11 scalefont def", "/head /Courier-Bold findfont 14 scalefont def"]
    pages = []
    for chunk in open(src).read().split("\f"):
        lines = chunk.strip("\n").split("\n")
        while lines:  # overflow onto extra pages
            pages.append(lines[:48]); lines = lines[48:]
    for n, lines in enumerate(pages, 1):
        y = 720
        for i, line in enumerate(lines):
            ps.append(f"{'head' if i == 0 and n == 1 else 'body'} setfont 72 {y} moveto ({esc(line)}) show")
            y -= 20 if i == 0 and n == 1 else 13.5
        ps.append(f"body setfont 290 40 moveto (- {n} -) show showpage")
    with tempfile.NamedTemporaryFile("w", suffix=".ps") as f:
        f.write("\n".join(ps) + "\n"); f.flush()
        subprocess.run(["ps2pdf", "-sPAPERSIZE=letter", f.name, dest], check=True)


def image_pdf(images, dest):
    """One image per page, page sized to the image (JPEG via Ghostscript's viewjpeg)."""
    vj = next((os.path.join(r, "viewjpeg.ps") for r, _, fs in os.walk("/usr/share/ghostscript") if "viewjpeg.ps" in fs), None)
    if not vj or not images:
        return False
    with tempfile.TemporaryDirectory() as tmp:
        cmds = []
        for i, img in enumerate(images):
            jpg = os.path.join(tmp, f"{i}.jpg")
            subprocess.run(["convert", img + "[0]", "-background", "white", "-flatten", "-resize", "1400x1400>", jpg], check=True)
            w, h = map(int, subprocess.run(["identify", "-format", "%w %h", jpg], capture_output=True, text=True).stdout.split())
            s = 612 / max(w, h)
            cmds.append(f"<< /PageSize [{w * s:.0f} {h * s:.0f}] >> setpagedevice {s} {s} scale ({jpg}) viewJPEG showpage")
        subprocess.run(["gs", "-q", "-dNOSAFER", "-sDEVICE=pdfwrite", "-o", dest, vj, "-c", " ".join(cmds)], check=True)
    return True


def drums(dest):
    """Doom, doom: four slow low thumps, then a fifth."""
    rate, out = 22050, bytearray()
    for beat in [0, 1, 2, 3, 3.6]:
        start = int(beat * rate)
        out.extend(b"\0\0" * max(0, start - len(out) // 2))
        for i in range(int(0.5 * rate)):
            t = i / rate
            out += struct.pack("<h", int(24000 * math.exp(-t * 7) * math.sin(2 * math.pi * (55 - 20 * t) * t)))
    with wave.open(dest, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate); w.writeframes(bytes(out))


def main():
    shutil.rmtree(ROOT, ignore_errors=True)
    base = os.path.join(ROOT, ME)
    made = []
    for r, _, fs in os.walk(os.path.join(CONTENT, ME)):
        for name in fs:
            src = os.path.join(r, name)
            rel = os.path.relpath(src, os.path.join(CONTENT, ME))
            if rel.endswith(".pdf.src"):
                rel = rel[:-4]
                os.makedirs(os.path.dirname(os.path.join(base, rel)), exist_ok=True)
                text_pdf(src, os.path.join(base, rel))
            else:
                os.makedirs(os.path.dirname(os.path.join(base, rel)), exist_ok=True)
                shutil.copyfile(src, os.path.join(base, rel))
            made.append(rel)
    missing = 0
    for it in json.load(open(os.path.join(S, "images.json"))):
        a = os.path.join(ASSETS, it["path"])
        if not os.path.exists(a):
            missing += 1; continue
        os.makedirs(os.path.dirname(os.path.join(base, it["path"])), exist_ok=True)
        shutil.copyfile(a, os.path.join(base, it["path"])); made.append(it["path"])
    if os.path.exists(os.path.join(ASSETS, "CREDITS.md")):
        shutil.copyfile(os.path.join(ASSETS, "CREDITS.md"), os.path.join(base, "CREDITS.md")); made.append("CREDITS.md")
    maps = os.path.join(base, "05_rivendell/library/maps")
    atlas = [os.path.join(maps, n) for n in ("misty-mountains.svg", "carta-marina.jpg", "moria-west-gate.png") if os.path.exists(os.path.join(maps, n))]
    if image_pdf(atlas, os.path.join(maps, "atlas.pdf")):
        made.append("05_rivendell/library/maps/atlas.pdf")
    os.makedirs(os.path.join(base, "07_moria/mazarbul-chamber"), exist_ok=True)
    drums(os.path.join(base, "07_moria/mazarbul-chamber/drums-in-the-deep.wav")); made.append("07_moria/mazarbul-chamber/drums-in-the-deep.wav")
    with open(os.path.join(base, "07_moria/durins-bridge.dwg"), "wb") as f:
        f.write(b"AC1032" + bytes(122) + b"one span, no rail" + bytes(64)); made.append("07_moria/durins-bridge.dwg")

    for rel in made:
        t = now - age(rel); os.utime(os.path.join(base, rel), (t, t))
    for r, ds, _ in os.walk(ROOT, topdown=False):
        for d in ds:
            os.utime(os.path.join(r, d), (now - 2000 * D, now - 2000 * D))
    print(f"{base}: {len(made)} files" + (f", {missing} images missing (run fetch_images.py)" if missing else ""))


if __name__ == "__main__":
    main()
