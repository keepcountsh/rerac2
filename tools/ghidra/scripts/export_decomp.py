#!/usr/bin/env python3
# Not re-run since the reorg (2026-09-27): moved from decomp/scripts/, paths updated (names: tools/ghidra/names/,
# decompiler export: work/decomp/, import ELFs: work/ghidra-import/). See tools/ghidra/README.md.
"""Bulk-exports decompiled C for every function of a program open in Ghidra to
work/decomp/<program>/<address>_<name>.c plus an index.tsv, so the code can be
grepped and read without round-tripping through the GUI. Re-run after renames.
"""
import os, sys, re, urllib.error
sys.path.insert(0, os.path.dirname(__file__))
from ghidra_http import get, post

program = sys.argv[1] if len(sys.argv) > 1 else "SCUS_971.99"
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))  # repo root (tools/ghidra/scripts/)
out = os.path.join(ROOT, "work/decomp", sys.argv[2] if len(sys.argv) > 2 else re.sub(r"[^A-Za-z0-9_.-]", "_", program))
os.makedirs(out, exist_ok=True)

# The plugin's API: before 7.0 `list_functions` + `decompile_function`; from 7.0 `find_functions` (paged) +
# `get_functions` (up to 20 decompiles per call).
try:
    funcs = get("list_functions", program=program, limit=20000)["functions"]
    v7 = False
except urllib.error.HTTPError as e:
    if e.code != 404:
        raise
    v7, funcs = True, []
    while True:
        page = get("find_functions", program=program, offset=len(funcs), limit=1000)
        funcs += page["functions"]
        if not page["functions"] or len(funcs) >= page["total"]:
            break

def decompiled(chunk):
    """[(function, decompiled text or None, the raw reply)] for a list of functions."""
    if not v7:
        f = chunk[0]
        res = get("decompile_function", address="0x" + f["address"].lower(), program=program, timeout=120)
        text = res.get("decompiled") or res.get("code") or res.get("result") if isinstance(res, dict) else res
        return [(f, text, res)]
    res = get("get_functions", program=program, functions=",".join(f["address"] for f in chunk), fields="decompiled_code")
    got = res.get("functions", {}) if isinstance(res, dict) else {}
    return [(f, (got.get(f["address"]) or {}).get("decompiled_code"), got.get(f["address"], res)) for f in chunk]

index = []
ok = fail = 0
step = 20 if v7 else 1
for i in range(0, len(funcs), step):
    for f, text, res in decompiled(funcs[i:i + step]):
        addr, name = f["address"].lower(), f["name"]
        if not isinstance(text, str) or not text.strip():
            text = str(res)
            fail += 1
        else:
            ok += 1
        fname = f"{addr}_{re.sub(r'[^A-Za-z0-9_]', '_', name)}.c"
        with open(os.path.join(out, fname), "w", encoding="utf-8", newline="\n") as fh:
            fh.write(f"// {program} {addr} {name}\n{text.replace(chr(13), '')}\n")
        index.append(f"{addr}\t{name}\t{fname}")
        if (ok + fail) % 200 == 0:
            print(f"...{ok + fail}/{len(funcs)}")
with open(os.path.join(out, "index.tsv"), "w", encoding="utf-8", newline="\n") as fh:
    fh.write("\n".join(index) + "\n")
print(f"{program}: exported {ok} functions, {fail} without decompiler output, to {out}")
