# demo video

    python3 fetch_images.py          # once: Wikimedia Commons images -> assets/ (gitignored, ~19 MB)
    python3 make_fixture.py          # demo/middle-earth: content/ + assets/ + generated PDFs/wavs, staggered mtimes
    python3 record.py                # drives ../target/release/tb in a private tmux, ~85 s -> caps.jsonl
    ~/venv/bin/python render.py      # 1080p PNGs + list.txt (needs Pillow, DejaVu fonts; Noto Color Emoji optional)
    ffmpeg -f concat -safe 0 -i list.txt -vf "fps=60,format=yuv420p" \
      -c:v libx264 -preset slow -crf 18 -tune animation -movflags +faststart tb-demo.mp4
    python3 reel.py tb-demo.mp4 ../docs/demo.gif   # README GIF: ~38 s highlight reel (cuts keyed to captions)

Captures the real terminal ~250 times a second, so the video shows actual animation timing.

## fixture

`content/middle-earth/` holds every text file (committed); `*.pdf.src` become
multi-page PDFs (form feed = page break). `make_fixture.py` also
synthesises two WAVs (Moria's `drums-in-the-deep.wav`, Fangorn's
`entish-hum.wav`) and a stub `.dwg`. `images.json` lists the Commons
file for each image path; `fetch_images.py` downloads 1600 px versions,
converts them to the extension the path asks for (jpg/png/webp/gif/tif) and
writes `CREDITS.md` (copied into the fixture) from Commons' license metadata.
Needs ImageMagick, Ghostscript (`ps2pdf`, `viewjpeg.ps`), network once.

The fixture is its own git repo (branch `journey`), committed once and then
left with one modified, one staged and one untracked file, so tb shows its
`M` `+` `?` markers and `d` diffs. `record.py` checks that state before each
take, runs tb with `HOME` set to `demo/demo` (a plain prompt for `!` and `s`,
settings reset every take) and aborts if any move lands on the wrong file.
