# Moved from decomp/scripts/ in the reorg (2026-09-27); unchanged. Used by every script here.
"""Tiny client for the GhidraMCP plugin's HTTP server (default 127.0.0.1:8089; GHIDRA_MCP_URL overrides it, as for
the plugin's MCP bridge, when the plugin's Server Port option is set elsewhere)."""
import json, os, urllib.request, urllib.parse

BASE = os.environ.get("GHIDRA_MCP_URL", "http://127.0.0.1:8089").rstrip("/")

def get(endpoint, **params):
    url = f"{BASE}/{endpoint}"
    if params:
        url += "?" + urllib.parse.urlencode(params)
    with urllib.request.urlopen(url, timeout=120) as r:
        body = r.read().decode()
    try:
        return json.loads(body)
    except json.JSONDecodeError:
        return body

def post(endpoint, **body):
    data = json.dumps(body).encode()
    req = urllib.request.Request(f"{BASE}/{endpoint}", data=data, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            text = r.read().decode()
    except urllib.error.HTTPError as e:
        text = e.read().decode()
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return text
