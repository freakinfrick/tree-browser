# demo video

    python3 make_fixture.py          # demo/middle-earth, staggered mtimes
    python3 record.py                # drives ~/.cargo/bin/tb in a private tmux, ~42 s -> caps.jsonl
    ~/venv/bin/python render.py      # 1080p PNGs + list.txt (needs Pillow, DejaVu Sans Mono)
    ffmpeg -f concat -safe 0 -i list.txt -vf "fps=60,format=yuv420p" \
      -c:v libx264 -preset slow -crf 18 -tune animation -movflags +faststart tb-demo.mp4

Captures the real terminal ~270x/s, so the video shows actual animation timing.
