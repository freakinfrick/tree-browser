#!/usr/bin/env python3
"""Drive tb through a scripted demo in a private tmux and capture the screen
continuously. Output: caps.jsonl of {t, cap, ansi}."""
import json, os, subprocess, threading, time

S = os.path.dirname(os.path.abspath(__file__))
DEMO = os.path.join(S, "demo", "middle-earth")
T = ["tmux", "-L", "tbdemo"]
W, H = 96, 25
TB = os.path.join(S, "..", "target", "release", "tb")  # cargo build --release first

def tmux(*a):
    return subprocess.run(T + list(a), capture_output=True, text=True).stdout

caption = ""
caps, stop = [], threading.Event()

def capture(t0):
    while not stop.is_set():
        caps.append({"t": time.time() - t0, "cap": caption, "ansi": tmux("capture-pane", "-ept", "d")})

def key(*ks, gap=0.32):
    for k in ks:
        if len(k) == 1:
            tmux("send-keys", "-t", "d", "-l", k)
        else:
            tmux("send-keys", "-t", "d", k)
        time.sleep(gap)

def say(text, hold=0.0):
    global caption
    caption = text
    time.sleep(hold)

_log = os.path.join(DEMO, "15_mordor/mount-doom/.eye-is-watching.log"); _n = time.time() - 90; os.utime(_log, (_n, _n))
subprocess.run(T + ["kill-server"], capture_output=True)
tmux("new", "-d", "-s", "d", "-x", str(W), "-y", str(H),
     f"sleep 0.6; env TERM=xterm-256color COLORTERM=truecolor {TB} {DEMO}")
t0 = time.time()
th = threading.Thread(target=capture, args=(t0,)); th.start()

say("tb  —  a horizontal tree file browser  ·  Rust + ratatui", 2.4)
say("color = newest change anywhere inside  ·  red = minutes  →  blue = years", 2.6)
say("the line stays put: j/k scroll the column through it")
key(*"jjjjjjjjjj", gap=0.28); time.sleep(0.6)                      # 04_bree -> 15_mordor
say("Mordor glows: one log deep inside changed 90 seconds ago")
key("l", gap=1.0); key("l", gap=1.2)                               # mordor -> barad-dur -> eye-uptime.prom
say("images preview as pictures (kitty / sixel / iTerm2, or half-blocks anywhere)")
key("j", "j", gap=0.45); key("Enter", gap=3.0); key("q", gap=0.9)  # pandemonium.webp
key("h", "h", gap=0.6)                                             # -> 15_mordor
say("many branches stay open at once")
key("j", gap=0.5); key("l", gap=0.9); key("j", "j", gap=0.4); key("l", gap=0.9)   # 07_moria -> mazarbul-chamber
key("j", "j", gap=0.4); key("Enter", gap=2.6); key("q", gap=0.9)   # durins-bane-sketch.gif
key("h", "h", gap=0.6)                                             # -> 07_moria
say("markdown rendered through glow")
key("j", gap=0.6); key("Enter", gap=1.4); key("j", "j", "j", "j", "j", gap=0.25); time.sleep(0.8)
key("q", gap=0.9)                                                  # README.md
say("PDFs page by page: j / k flip")
key("j", gap=0.5); key("l", gap=0.9); key("l", gap=0.9)            # 05_rivendell -> council-minutes -> transcript.pdf
key("Enter", gap=2.4); key("j", gap=1.6); key("j", gap=1.6); key("j", gap=1.8); key("q", gap=0.9)
key("h", "h", gap=0.6)                                             # -> 05_rivendell
say("code through bat, with syntax highlighting")
key("j", "j", gap=0.4); key("l", gap=0.9); key("j", "j", "j", gap=0.35); key("Enter", gap=2.2); key("q", gap=0.9)
say("collapse: branches fold back in and fade")
key("h", gap=0.6); key("c", gap=1.6)
say("? = help overlay")
key("?", gap=2.4); key("?", gap=0.8)
say("- = re-root one level up; the whole tree slides over")
key("-", gap=1.6)
say("tb  ·  ~/tree-browser", 1.6)
stop.set(); th.join()
tmux("send-keys", "-t", "d", "q"); time.sleep(0.3)
subprocess.run(T + ["kill-server"], capture_output=True)
with open(os.path.join(S, "caps.jsonl"), "w") as f:
    for c in caps:
        f.write(json.dumps(c) + "\n")
dur = caps[-1]["t"]
print(f"{len(caps)} captures over {dur:.1f}s = {len(caps)/dur:.0f}/s")
