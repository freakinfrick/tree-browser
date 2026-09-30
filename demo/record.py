#!/usr/bin/env python3
"""Drive tb through a scripted demo in a private tmux and capture the screen
continuously. Output: caps.jsonl of {t, cap, ansi}."""
import json, os, shutil, subprocess, tempfile, threading, time

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

def expect(name):
    """Assert the status bar's cursor name starts with `name`; abort the take if not."""
    bar = tmux("capture-pane", "-pt", "d").rstrip("\n").split("\n")[-1]
    shown = bar.rpartition("› ")[2].partition("⎇")[0].strip().rstrip("…")  # truncated to fit
    if len(shown) < 4 or not name.startswith(shown):
        stop.set(); th.join(); subprocess.run(T + ["kill-server"], capture_output=True)
        raise SystemExit(f"choreography off: expected {name!r}, status bar: {bar.strip()!r}")

# Heat hook: a visible log deep in Mordor (dotfiles don't count toward heat), aged
# so it reads ~90 s old when the route reaches Mordor ~45 s into the take.
_log = os.path.join(DEMO, "15_mordor/barad-dur/structural-failure.log"); _n = time.time() - 45; os.utime(_log, (_n, _n))
_cfg = tempfile.mkdtemp()  # empty config dir: default sort, no user settings
subprocess.run(T + ["kill-server"], capture_output=True)
tmux("new", "-d", "-s", "d", "-x", str(W), "-y", str(H),
     f"sleep 0.6; env XDG_CONFIG_HOME={_cfg} TERM=xterm-256color COLORTERM=truecolor {TB} {DEMO}")
t0 = time.time()
th = threading.Thread(target=capture, args=(t0,)); th.start()

# The route follows the journey, 01 -> 16. Cursor starts on 01_lonely-mountain;
# l into a folder lands on its first entry.
say("tb  —  a horizontal tree file browser  ·  Rust + ratatui", 2.4)
expect("01_lonely-mountain")
say("color = newest change anywhere inside  ·  red = minutes  →  blue = years", 2.6)
say("the line stays put: j/k scroll the column through it")
key(*"jjjj", gap=0.34); time.sleep(0.6); expect("05_rivendell")
say("PDFs page by page: j / k flip")
key("l", gap=0.9); key("j", gap=0.5); key("l", gap=0.9)            # arwen.md -> council-minutes -> transcript.pdf
expect("council-of-elrond-transcript.pdf")
key("Enter", gap=2.4); key("j", gap=1.6); key("j", gap=1.6); key("j", gap=1.8); key("q", gap=0.9)
key("h", "h", gap=0.6); expect("05_rivendell")
say("many branches stay open at once")
key("j", "j", gap=0.5); expect("07_moria")
key("l", gap=0.9); key("j", "j", "j", gap=0.4); key("l", gap=0.9)  # balrog-incident-report.md -> mazarbul-chamber -> book-of-records.txt
key("j", "j", gap=0.4); expect("durins-bane-sketch.gif")
key("Enter", gap=2.6); key("q", gap=0.9)
key("h", gap=0.6); key("j", gap=0.6); expect("README.md")           # 07_moria/README.md
say("markdown rendered through glow")
key("Enter", gap=1.4); key("j", "j", "j", "j", "j", gap=0.25); time.sleep(0.8)
key("q", gap=0.9)
key("h", gap=0.6); expect("07_moria")
say("code through bat, with syntax highlighting")
key("j", "j", "j", "j", gap=0.3); expect("11_rohan")
key("l", gap=0.9); key("j", gap=0.4); key("l", gap=0.9)            # edoras -> helms-deep -> culvert-bug.md
key("j", "j", gap=0.35); expect("wall-repairs.rs")
key("Enter", gap=2.2); key("q", gap=0.9)
key("h", "h", gap=0.6); expect("11_rohan")
say("Mordor glows: one log deep inside changed 90 seconds ago")
key("j", "j", "j", "j", gap=0.3); time.sleep(0.6); expect("15_mordor")
key("l", gap=1.0); key("l", gap=1.0); key("j", "j", gap=0.45)      # barad-dur -> alerts.yml -> eye-uptime.prom
expect("eye-uptime.prom")
say("images preview as pictures (kitty / sixel / iTerm2, or half-blocks anywhere)")
key("j", "j", "j", "j", gap=0.35); expect("pandemonium.webp")
key("Enter", gap=3.0); key("q", gap=0.9)
key("j", gap=1.4); expect("structural-failure.log")                # the 90-second log
say("collapse: branches fold back in and fade")
key("h", gap=0.6); expect("barad-dur"); key("c", gap=1.6)
say("? = help overlay")
key("?", gap=2.4); key("?", gap=0.8)
say("- = re-root one level up; the whole tree slides over")
key("-", gap=1.6)
say("tb  ·  ~/tree-browser", 1.6)
stop.set(); th.join()
tmux("send-keys", "-t", "d", "q"); time.sleep(0.3)
subprocess.run(T + ["kill-server"], capture_output=True)
shutil.rmtree(_cfg, ignore_errors=True)
with open(os.path.join(S, "caps.jsonl"), "w") as f:
    for c in caps:
        f.write(json.dumps(c) + "\n")
dur = caps[-1]["t"]
print(f"{len(caps)} captures over {dur:.1f}s = {len(caps)/dur:.0f}/s")
