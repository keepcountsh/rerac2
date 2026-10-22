#!/usr/bin/env python3
"""Going Commando's census tag table (docs/plan/rac2_class_census.md, "Method" step 4).

Writes tools/ghidra/names/census_systems.tsv from:
  * work/census/shared.tsv (a census run without tags: every shared function and global the unported units use);
  * the citations of each address in R&C2's port and docs (crates/rc-game, crates/rc-engine/src/rac2*,
    crates/rc-formats/src/rac2, docs/plan/rac2*): each citing line is read; a line that says the thing is not ported
    (not ported / not yet / later / not drawn / not modelled / waits / stub) counts against, any other line of code
    for. One line of code for: has (the port names it as its model); only lines against: missing; only doc mentions:
    left for the manual table. The citing file names the system;
  * the manual rows of tools/census/tags_manual.tsv (read in the decomp: they win over the generated ones).
Uncited functions stay untagged unless a manual row or a range rule (the vector maths 0x2ee000..0x2f0200: trivial)
places them. Run from the repo root:  python3 tools/census/tags.py [path/to/shared.tsv]
"""
import os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CITES = ["crates/rc-game/", "crates/rc-engine/src/rac2", "crates/rc-formats/src/rac2", "docs/plan/rac2"]
# Files that came from R&C1's port with its level01 / boot addresses (docs/plan/porting_from_rac1.md): their numbers
# are not Going Commando's.
RAC1_ADDRESSED = ["crates/rc-game/src/audio", "crates/rc-game/src/collision", "crates/rc-game/src/hero/anim.rs",
                  "crates/rc-game/src/hero/quat.rs", "crates/rc-game/src/pad.rs", "crates/rc-game/src/particles/pool.rs",
                  "crates/rc-game/src/particles/type23.rs", "crates/rc-game/src/particles/type48.rs",
                  "crates/rc-game/src/point_lights.rs", "crates/rc-game/src/ps2v.rs", "crates/rc-game/src/scene_camera.rs",
                  "crates/rc-game/src/spline.rs"]
NEG = re.compile(r"not ported|not yet|\blater\b|not drawn|not modell?ed|waits? for|\bstub|isn't ported|not read yet|left out|not wired|unported", re.I)

# The system a citing file stands for (first match).
SYSTEMS = [
    ("rc-game/src/hero/items", "items"), ("rc-game/src/hero/layers", "anim"), ("rc-game/src/hero/", "hero"),
    ("rc-game/src/hero.rs", "hero"), ("rc-game/src/moby/ai", "enemy"), ("rc-game/src/moby/hits", "hits"),
    ("rc-game/src/moby/attach", "group"), ("rc-game/src/moby/shards", "breakfx"), ("rc-game/src/moby/bolts", "bolts"),
    ("rc-game/src/moby/aim", "weapons"), ("rc-game/src/moby/classes/lancer", "weapons"), ("rc-game/src/moby/classes/shot", "weapons"),
    ("rc-game/src/moby/classes/wrench", "weapons"), ("rc-game/src/moby/classes/enemies", "enemy"),
    ("rc-game/src/moby/classes/carriers", "group"), ("rc-game/src/moby/classes/", "moby"), ("rc-game/src/moby", "moby"),
    ("rc-game/src/particles", "particles"), ("rc-game/src/fx", "fxdraw"), ("rc-game/src/canopy", "fxdraw"),
    ("rc-game/src/camera", "camera"), ("rc-game/src/cameras", "camera"), ("rc-game/src/hud", "ui"), ("rc-game/src/gui", "ui"),
    ("rc-game/src/help", "ui"), ("rc-game/src/front_end", "ui"), ("rc-game/src/scene", "scene"), ("rc-game/src/level", "level"),
    ("rc-game/src/paths", "path"), ("rc-game/src/rng", "rng"), ("rc-game/src/game", "game"), ("rc-game/src/light", "light"),
    ("rc-game/src/sky", "sky"), ("rc-engine/src/rac2_mobys", "moby"), ("rc-engine/src/rac2_play", "game"),
    ("rc-engine/src/rac2_level", "level"), ("rc-formats/src/rac2", "formats"), ("docs/plan/rac2", "docs"),
]


def system_of(path):
    p = path.replace("crates/", "")
    for k, s in SYSTEMS:
        if p.startswith(k) or path.startswith(k):
            return s
    return "other"


def files():
    for top in ["crates", "docs"]:
        for d, _, fs in os.walk(os.path.join(ROOT, top)):
            for f in fs:
                if not (f.endswith(".rs") or f.endswith(".md")):
                    continue
                p = os.path.relpath(os.path.join(d, f), ROOT)
                if any(p.startswith(c) for c in CITES) and not any(p.startswith(x) for x in RAC1_ADDRESSED):
                    yield p


def citations():
    """address -> [(file, line text)]"""
    tok = re.compile(r"(?<![0-9A-Za-z])0x([0-9a-f]{5,8})(?![0-9A-Za-z])")
    out = {}
    for p in files():
        with open(os.path.join(ROOT, p), encoding="utf-8", errors="replace") as fh:
            for line in fh:
                for m in tok.finditer(line.lower()):
                    out.setdefault(int(m.group(1), 16), []).append((p, line.strip()))
    return out


def infer(shared, rows):
    """The rules for the rest, to a fixpoint: a leaf of at most 12 words is trivial (leaf); a level copy with a near
    L00 twin (the census's "near" column) takes the twin's tag (near); a function of at most 12 words with one callee
    takes the callee's (wrap)."""
    tag = {r[0]: r for r in rows}
    info = {}
    for l in list(open(shared))[1:]:
        c = l.rstrip("\n").split("\t")
        callees = [x for x in c[15].split(";") if x] if len(c) > 15 else []
        near = c[16].split("@")[0] if len(c) > 16 and c[16] else None
        info[c[0]] = (c[1], int(c[7] or 0), callees, near)
    changed = True
    while changed:
        changed = False
        for key, (name, words, callees, near) in info.items():
            if key in tag:
                continue
            got = None
            if 0 < words <= 12 and not callees:
                got = [key, name, "trivial", "trivial", "leaf"]
            elif near and near in tag:
                t = tag[near]
                got = [key, name, t[2], t[3], "near"]
            elif 0 < words <= 12 and len(callees) == 1 and callees[0] in tag:
                t = tag[callees[0]]
                got = [key, name, t[2], t[3], "wrap"]
            elif 0 < words <= 40 and callees and all(x in tag for x in callees):
                # Every callee placed and at most one system past the maths: that system's tag.
                sys_ = {(tag[x][2], tag[x][3]) for x in callees if tag[x][3] != "trivial"}
                if len(sys_) <= 1:
                    s, st = next(iter(sys_)) if sys_ else ("math", "trivial")
                    got = [key, name, s, st, "calls"]
            if got:
                tag[key] = got
                rows.append(got)
                changed = True


def main():
    shared = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "work/census/shared.tsv")
    cites = citations()
    manual = {}
    mpath = os.path.join(ROOT, "tools/census/tags_manual.tsv")
    if os.path.exists(mpath):
        for l in open(mpath):
            if l.startswith("#") or not l.strip() or l.startswith("key\t"):
                continue
            c = l.rstrip("\n").split("\t")
            manual[c[0]] = c
    rows = []
    seen = set()
    for l in list(open(shared))[1:]:
        c = l.rstrip("\n").split("\t")
        key, name = c[0], c[1]
        seen.add(key)
        if key in manual:
            rows.append(manual[key])
            continue
        addr = None
        m = re.match(r"(?:L00|boot):([0-9a-f]{8})$", key) or re.match(r"G:([0-9a-f]{8})$", key)
        if m:
            addr = int(m.group(1), 16)
        if addr is not None and 0x2EE000 <= addr < 0x2F0200 and key.startswith("L00:"):
            rows.append([key, name or "vector_math", "math", "trivial", "range"])
            continue
        cs = cites.get(addr, []) if addr is not None else []
        if not cs:
            continue
        pos = [x for x in cs if not NEG.search(x[1]) and x[0].endswith(".rs")]
        neg = [x for x in cs if NEG.search(x[1])]
        if not pos and not neg:
            continue  # only described in the docs: read by hand
        status = "has" if pos else "missing"
        sysvotes = {}
        for p, _ in (pos or neg):
            s = system_of(p)
            if s != "docs":
                sysvotes[s] = sysvotes.get(s, 0) + 1
        system = max(sysvotes, key=sysvotes.get) if sysvotes else system_of((pos or neg)[0][0])
        rows.append([key, name, system, status, "cite"])
    for k, c in manual.items():
        if k not in seen:
            rows.append(c)
    infer(shared, rows)
    out = os.path.join(ROOT, "tools/ghidra/names/census_systems.tsv")
    with open(out, "w") as fh:
        fh.write("# Going Commando census tags (tools/census/tags.py; manual rows: tools/census/tags_manual.tsv).\n")
        fh.write("# conf: cite = from the port's citing lines; range = the vector maths range; L = read in the decomp, inferred;\n")
        fh.write("#   leaf = a leaf of at most 12 words; near = a level copy's L00 twin; wrap = a thin wrapper's callee;\n")
        fh.write("#   calls = at most 40 words whose callees are all placed, with at most one system past the maths.\n")
        fh.write("key\tname\tsystem\tstatus\tconf\n")
        for r in rows:
            fh.write("\t".join(r[:5]) + "\n")
    print(f"{len(rows)} tags -> {out}")


if __name__ == "__main__":
    main()
