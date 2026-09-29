#!/usr/bin/env python3
"""Build demo/demo/middle-earth: a small tree with staggered mtimes.
Folder mtimes are forced old, so any heat a folder shows comes from inside."""
import os, shutil, time

S = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(S, "demo")
now, H, D = time.time(), 3600, 86400
tree = {
 "middle-earth": {
  "README.md": 2*H,
  "the-shire": {"bag-end": {"second-breakfast.md": 30*D, "pipe-weed-ledger.csv": 90*D, "mathom-inventory.txt": 400*D},
                "green-dragon-inn": {"tab.csv": 3*D, "songs.txt": 200*D}, "party-tree.jpg": 800*D},
  "rivendell": {"council-minutes": {"session-01.md": 20*D, "session-02.md": 19*D, "who-carries-the-ring.md": 6*D},
                "library": {"red-book.txt": 700*D, "maps": {"misty-mountains.svg": 300*D, "moria-west-gate.png": 1200*D}},
                "elrond-notes.md": 45*D},
  "moria": {"mazarbul-chamber": {"book-of-records.txt": 1500*D, "drums-in-the-deep.wav": 1500*D}, "durins-bridge.dwg": 1600*D},
  "lothlorien": {"mirror-of-galadriel": {"visions.log": 25*60, "not-yet-come-to-pass.md": 40*60}, "mallorn-census.csv": 60*D, "cloaks.txt": 10*D},
  "rohan": {"edoras": {"horse-parking.md": 12*D, "eomer-attendance.csv": 12*D}, "helms-deep": {"wall-repairs.rs": 5*H, "culvert-bug.md": 4*H}},
  "gondor": {"minas-tirith": {"beacon-schedule.toml": 8*D, "stewards-succession.md": 150*D, "palantir-screensaver.png": 900*D}, "osgiliath.txt": 365*D},
  "mordor": {"mount-doom": {".eye-is-watching.log": 90, "one-does-not-simply.txt": 2*D}, "barad-dur": {"orc-hr": {"shift-roster.csv": 1*D, "complaints-to-sauron.txt": 7*H}}},
  "src": {"main.rs": 20*60, "layout.rs": 45*60, "anim.rs": 10*60},
  "grey-havens": {"packing-list.md": 1000*D, "last-boat-timetable.csv": 1100*D},
 }}
content = {
 "README.md": "# Middle-earth\n\nA **demo tree** for `tb`.\n\n## Regions\n\n- The Shire — *second breakfast*\n- Rivendell — council minutes\n- Mordor — do not simply walk in\n\n> Not all those who wander are lost.\n\n```rust\nfn main() { println!(\"one ring\"); }\n```\n",
 "main.rs": "use std::collections::HashMap;\n\n/// Where is everyone?\nfn main() {\n    let mut fellowship = HashMap::new();\n    for (who, at) in [(\"Frodo\", \"Mordor\"), (\"Sam\", \"Mordor\"), (\"Aragorn\", \"Gondor\")] {\n        fellowship.insert(who, at);\n    }\n    println!(\"{fellowship:?}\");\n}\n",
}
dirs = []
def mk(base, node):
    for name, v in node.items():
        p = os.path.join(base, name)
        if isinstance(v, dict):
            os.makedirs(p, exist_ok=True); mk(p, v); dirs.append(p)
        else:
            open(p, "w").write(content.get(name, f"{name}\n" * 3)); os.utime(p, (now - v, now - v))
shutil.rmtree(ROOT, ignore_errors=True)
mk(ROOT, tree)
for p in dirs:
    os.utime(p, (now - 2000 * D, now - 2000 * D))
print(ROOT)
