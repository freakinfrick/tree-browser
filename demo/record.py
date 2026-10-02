#!/usr/bin/env python3
"""Drive tb through a scripted demo in a private tmux and capture the screen
continuously. Output: caps.jsonl of {t, cap, ansi}.

The route follows the journey, 01 -> 16, and shows one feature per stop.
Every move is checked against the status bar (expect), so a fixture or
binary change that shifts the tree aborts the take instead of recording
the wrong file. Run make_fixture.py first; the take is ~90 s."""
import json, os, shutil, subprocess, sys, threading, time

S = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, S); sys.dont_write_bytecode = True
import make_fixture as mf

HOME = os.path.join(S, "demo")  # the fixture's parent: paths read ~/middle-earth/...
DEMO = os.path.join(HOME, "middle-earth")
T = ["tmux", "-L", "tbdemo"]
W, H = 96, 25
TB = os.path.join(S, "..", "target", "release", "tb")  # cargo build --release first
GIT_STORY = {"M  11_rohan/helms-deep/culvert-bug.md", " M 11_rohan/helms-deep/wall-repairs.rs",
             "?? 08_lothlorien/mirror-of-galadriel/not-yet-come-to-pass.md"}
RIPPLE = "07_moria/mazarbul-chamber/book-of-records.txt"   # touched mid-take
HOT_LOG = "15_mordor/barad-dur/structural-failure.log"     # reads ~1m old on arrival
RFC = "10_fangorn/treebeard/rfcs/0001-rename-from-tree-browser.md"
VOTE = "10_fangorn/treebeard/entmoot/name-vote.csv"        # these two keep Fangorn red

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

def type_(text, gap=0.07):
    for ch in text:
        tmux("send-keys", "-t", "d", "-l", ch)
        time.sleep(gap)

def say(text, hold=0.0):
    global caption
    caption = text
    time.sleep(hold)

def screen():
    return tmux("capture-pane", "-pt", "d").rstrip("\n").split("\n")

def abort(why):
    stop.set(); th.join(); subprocess.run(T + ["kill-server"], capture_output=True)
    raise SystemExit(f"choreography off: {why}")

def expect(name):
    """Assert the status bar's cursor name starts with `name`; abort the take if not."""
    bar = screen()[-1]
    shown = bar.rpartition("› ")[2]  # the name, cut to fit, runs straight into notes and the branch
    same = len(os.path.commonprefix([shown, name]))
    if same < min(len(name), 5):
        abort(f"expected {name!r}, status bar: {bar.strip()!r}")

def wheel(name, ticks, gap=0.35):
    """Scroll the mouse wheel down over the on-screen entry `name`."""
    time.sleep(0.4)  # let the camera settle before reading positions
    for row, line in enumerate(screen()[:-1]):
        col = line.find(name)
        if col >= 0:
            break
    else:
        abort(f"{name!r} not on screen for the wheel")
    for _ in range(ticks):
        seq = f"\x1b[<65;{col + 2};{row + 1}M"
        tmux("send-keys", "-t", "d", "-H", *[f"{b:02x}" for b in seq.encode()])
        time.sleep(gap)

def touch(rel, age):
    t = time.time() - age
    os.utime(os.path.join(DEMO, rel), (t, t))

# Same take every run: fixture git story intact, mtimes reset, no settings left
# over from the last take's menu visit, a plain prompt for ! and s.
status = subprocess.run(["git", "-C", DEMO, "status", "--porcelain"], capture_output=True, text=True).stdout
if set(status.splitlines()) != GIT_STORY:
    raise SystemExit("fixture is stale or missing: run make_fixture.py")
touch(RIPPLE, mf.age(RIPPLE))
touch(HOT_LOG, 20)
touch(RFC, mf.age(RFC)); touch(VOTE, mf.age(VOTE))
shutil.rmtree(os.path.join(HOME, ".config"), ignore_errors=True)
with open(os.path.join(HOME, ".bashrc"), "w") as f:
    f.write("PS1='\\[\\e[1;32m\\]sam@bag-end\\[\\e[0m\\]:\\[\\e[1;34m\\]\\W\\[\\e[0m\\]\\$ '\n")
open(os.path.join(HOME, ".sudo_as_admin_successful"), "w").close()  # silences Ubuntu's sudo hint

subprocess.run(T + ["kill-server"], capture_output=True)
tmux("new", "-d", "-s", "d", "-x", str(W), "-y", str(H),
     f"sleep 0.6; env -u XDG_CONFIG_HOME HOME={HOME} SHELL=/bin/bash TERM=xterm-256color COLORTERM=truecolor {TB} {DEMO}")
t0 = time.time()
th = threading.Thread(target=capture, args=(t0,)); th.start()

say("treebeard  —  a horizontal tree file browser", 2.2)
expect("01_lonely-mountain")
say("color = newest change inside  ·  red = minutes → blue = years", 2.4)
say("j / k scroll the column  ·  the line stays put")
key(*"jjjj", gap=0.26); time.sleep(0.5); expect("05_rivendell")

say("l opens  ·  PDFs preview page by page")
key("l", gap=0.6); key("j", gap=0.35); key("l", gap=0.7)          # arwen.md -> council-minutes -> transcript.pdf
expect("council-of-elrond-transcript.pdf")
key("Enter", gap=1.8); key("j", gap=1.1); key("j", gap=1.1); key("q", gap=0.6)
key("h", "h", gap=0.4); expect("05_rivendell")

say("e explodes a folder: everything inside unfurls")
key("e", gap=2.4); expect("05_rivendell")

say("audio: waveform and scrubber")
key("Down", "Down", gap=0.3); expect("07_moria")  # arrows stay in the column; j would walk into the exploded folder
key("l", gap=0.5); key("j", "j", "j", gap=0.2); key("l", gap=0.5)  # balrog-incident-report.md -> mazarbul-chamber -> book-of-records.txt
key("j", gap=0.4); expect("drums-in-the-deep.wav")
key("Enter", gap=2.8); key("q", gap=0.5)

say("images and GIFs, inline")
key("j", gap=0.4); expect("durins-bane-sketch.gif")
key("Enter", gap=2.2); key("q", gap=0.6)

say("live: a file changes and its folders re-heat")
time.sleep(0.4); touch(RIPPLE, 0); time.sleep(2.8)                 # book-of-records.txt: Moria turns red

say("Fangorn glows: the rename RFC passed minutes ago")
key("h", "h", gap=0.4); expect("07_moria")
key("Down", "Down", "Down", gap=0.25); expect("10_fangorn"); time.sleep(0.5)
key("l", gap=0.35); key("G", gap=0.35); expect("treebeard")
key("l", gap=0.35)
key("/", gap=0.2); type_("rf"); time.sleep(0.2); key("Enter", gap=0.3); key("l", gap=0.5)  # Enter stays on rfcs/; l: 0000-template.md
key("j", gap=0.35); expect("0001-rename-from-tree-browser.md")
say("RFC 0001: the rename, read through glow")
key("Enter", gap=1.9); key("q", gap=0.4)
key("h", "h", "h", gap=0.22); expect("10_fangorn")
key("Up", "Up", "Up", gap=0.2); expect("07_moria")

say("the mouse wheel scrolls any column")
wheel("07_moria", 4); time.sleep(0.4); expect("11_rohan")

say("git: M modified  ·  + staged  ·  ? untracked")
key("l", gap=0.5); key("j", gap=0.35); key("l", gap=0.6)          # edoras -> helms-deep -> culvert-bug.md
expect("culvert-bug.md"); time.sleep(1.4)
say("markdown through glow  ·  d shows the git diff")
key("Enter", gap=1.4); key("j", "j", gap=0.3); key("d", gap=2.0); key("q", gap=0.6)
say("code through bat, syntax highlighted")
key("j", "j", gap=0.3); expect("wall-repairs.rs")
key("Enter", gap=1.8); key("d", gap=1.8); key("q", gap=0.6)

say("! runs a command here  ·  $f = the selection")
key("!", gap=0.3); type_('grep -n deeping "$f"'); time.sleep(0.3); key("Enter", gap=1.8); key("x", gap=0.7)
expect("wall-repairs.rs")
say("s opens a shell here  ·  exit comes back")
key("s", gap=0.9); type_("git status -s"); key("Enter", gap=1.8); type_("exit"); key("Enter", gap=1.0)
expect("wall-repairs.rs")

say("/ finds fuzzily in the column  ·  tab cycles matches")
key("h", "h", gap=0.4); expect("11_rohan")
key("/", gap=0.3); type_("mor", gap=0.15); time.sleep(0.9); key("Tab", gap=0.9); key("Enter", gap=0.5); key("l", gap=0.6)
expect("barad-dur")
say("Mordor burns: one log changed a minute ago")
key("l", gap=0.5); key("G", gap=1.8); expect("structural-failure.log")

say("o sorts: newest · largest · type · name")
key("h", "h", gap=0.5); expect("15_mordor")
key("o", gap=1.7); key("o", gap=1.2); key("o", gap=1.0); key("o", gap=0.9)
expect("15_mordor")

say(", settings apply as you change them  ·  seven heat palettes")
key(",", gap=0.6); key(*"jjjjjjjjjjjj", gap=0.12)                 # -> Accent
key("l", gap=0.8); key("l", gap=0.8); key("j", gap=0.4); key("l", "l", "l", "l", gap=1.0)   # teal, violet; heat colors: magma, neon, aurora, glacier
key("h", "h", "h", "h", gap=0.15); key("k", gap=0.2); key("h", "h", gap=0.3); key("Escape", gap=0.6)  # back to the defaults

say("C folds every other branch")
key("C", gap=1.6)
say("? lists every key")
key("?", gap=2.2); key("?", gap=0.6)
say("treebeard  ·  Rust + ratatui", 1.8)

stop.set(); th.join()
tmux("send-keys", "-t", "d", "q"); time.sleep(0.3)
subprocess.run(T + ["kill-server"], capture_output=True)
with open(os.path.join(S, "caps.jsonl"), "w") as f:
    for c in caps:
        f.write(json.dumps(c) + "\n")
dur = caps[-1]["t"]
print(f"{len(caps)} captures over {dur:.1f}s = {len(caps)/dur:.0f}/s")
