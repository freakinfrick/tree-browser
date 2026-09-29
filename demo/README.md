# demo video

    python3 fetch_images.py          # once: Wikimedia Commons images -> assets/ (gitignored, ~16 MB)
    python3 make_fixture.py          # demo/middle-earth: content/ + assets/ + generated PDFs/wav, staggered mtimes
    python3 record.py                # drives ~/.cargo/bin/tb in a private tmux, ~42 s -> caps.jsonl
    ~/venv/bin/python render.py      # 1080p PNGs + list.txt (needs Pillow, DejaVu Sans Mono)
    ffmpeg -f concat -safe 0 -i list.txt -vf "fps=60,format=yuv420p" \
      -c:v libx264 -preset slow -crf 18 -tune animation -movflags +faststart tb-demo.mp4

Captures the real terminal ~270x/s, so the video shows actual animation timing.

## fixture

`content/middle-earth/` holds every text file (committed); `*.pdf.src` become
multi-page PDFs (form feed = page break). `images.json` lists the Commons
file for each image path; `fetch_images.py` downloads 1600 px versions,
converts them to the extension the path asks for (jpg/png/webp/gif/tif) and
writes `CREDITS.md` (copied into the fixture) from Commons' license metadata.
Needs ImageMagick, Ghostscript (`ps2pdf`, `viewjpeg.ps`), network once.
