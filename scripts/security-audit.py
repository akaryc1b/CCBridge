#!/usr/bin/env python3
"""Audit resolved production/build dependency graphs against OSV; fail closed on errors."""
import argparse
import concurrent.futures
import datetime
import json
import pathlib
import re
import subprocess
import time
import urllib.error
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
SCOPES = {
    "runner-windows": ["--manifest-path", "Cargo.toml", "--no-default-features", "--target", "x86_64-pc-windows-msvc"],
    "runner-linux": ["--manifest-path", "Cargo.toml", "--no-default-features", "--target", "x86_64-unknown-linux-gnu"],
    "app-arm64": ["--manifest-path", "src-tauri/Cargo.toml", "--target", "aarch64-apple-darwin"],
    "app-x86_64": ["--manifest-path", "src-tauri/Cargo.toml", "--target", "x86_64-apple-darwin"],
}


def osv(endpoint, data=None):
    body = None if data is None else json.dumps(data).encode()
    for attempt in range(3):
        try:
            request = urllib.request.Request("https://api.osv.dev/v1/" + endpoint, data=body,
                headers={"Content-Type": "application/json", "User-Agent": "CCBridge-security-audit"})
            with urllib.request.urlopen(request, timeout=45) as response:
                return json.load(response)
        except (urllib.error.URLError, TimeoutError):
            if attempt == 2:
                raise
            time.sleep(attempt + 1)


def exceptions(path):
    entries = json.loads(path.read_text())
    today = datetime.date.today()
    for entry in entries:
        if not all(entry.get(k) for k in ["package", "version", "ids", "scopes", "reason", "reference", "reviewed", "expires"]):
            raise ValueError("Incomplete security exception")
        if datetime.date.fromisoformat(entry["expires"]) <= today:
            raise ValueError("Expired security exception: " + entry["package"])
    return entries


def classify(package, version, scope, advisory, entries):
    identifiers = {advisory["id"], *advisory.get("aliases", [])}
    for entry in entries:
        if entry["package"] == package and entry["version"] == version and scope in entry["scopes"] and identifiers.intersection(entry["ids"]):
            return entry["reason"]
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    reviewed = exceptions(ROOT / "security/advisory-exceptions.json")
    graphs = {}
    for scope, options in SCOPES.items():
        text = subprocess.check_output(["cargo", "tree", "--locked", "--edges", "normal,build", "--prefix", "none", "--format", "{p}", *options], cwd=ROOT, text=True)
        # Local path packages carry a path suffix and are not crates.io dependencies.
        graphs[scope] = set(re.findall(r"^(\S+) v(\S+)(?: \(\*\))?$", text, re.MULTILINE))
        # The reviewed rmcp exception is only valid while HTTP/OAuth/client features stay off.
        feature_text = subprocess.check_output(["cargo", "tree", "--locked", "--edges", "features", "--invert", "rmcp", "--prefix", "none", *options], cwd=ROOT, text=True)
        features = re.findall(r'^rmcp feature "([^"]+)"', feature_text, re.MULTILINE)
        if any("http" in f or "auth" in f or f == "client" for f in features):
            raise ValueError("rmcp HTTP/OAuth/client enabled: review the stdio-only exception before release")
    packages = sorted(set.union(*graphs.values()))
    matches = []
    for offset in range(0, len(packages), 150):
        chunk = packages[offset:offset + 150]
        result = osv("querybatch", {"queries": [{"package": {"ecosystem": "crates.io", "name": n}, "version": v} for n, v in chunk]})
        if len(result["results"]) != len(chunk):
            raise ValueError("OSV response count mismatch")
        for (name, version), result in zip(chunk, result["results"]):
            matches.extend((name, version, item["id"]) for item in result.get("vulns", []))
    identifiers = sorted({m[2] for m in matches})
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        details = dict(zip(identifiers, pool.map(lambda i: osv("vulns/" + i), identifiers)))
    rows = []
    for name, version, identifier in matches:
        advisory = details[identifier]
        if advisory.get("withdrawn"):
            continue
        for scope, packages in graphs.items():
            if (name, version) not in packages:
                continue
            reason = classify(name, version, scope, advisory, reviewed)
            rows.append({"scope": scope, "package": name, "version": version, "id": identifier,
                         "summary": advisory.get("summary", ""), "exception": reason})
    result = {"checked_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "graphs": {k: len(v) for k, v in graphs.items()}, "findings": rows}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    for row in rows:
        print(f"{'REVIEWED' if row['exception'] else 'FAIL'} {row['scope']}: {row['package']}@{row['version']} {row['id']} {row['exception'] or row['summary']}")
    blocked = sum(not row["exception"] for row in rows)
    print(f"Security audit: {len(graphs)} production/build graphs; {blocked} unreviewed findings")
    return 1 if blocked else 0


if __name__ == "__main__":
    raise SystemExit(main())
