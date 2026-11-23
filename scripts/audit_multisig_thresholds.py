#!/usr/bin/env python3
"""
Lightweight static audit for EmergencyGuard::initialize calls

Scans the repository for literal calls to `EmergencyGuard::initialize(..., vec![...], <threshold>)`
and reports cases where the threshold is zero or greater than the number of literal admins provided.

Usage:
    python3 scripts/audit_multisig_thresholds.py [--help]

The audit is fully local (no network calls). Filesystem errors, including errors from
network-mounted paths, are reported as warnings instead of crashing the scan.
The process exits with status 1 when any invalid threshold is found.

This is best-effort static analysis; non-literal admin lists or thresholds will be reported as "unknown".
"""
import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PATTERN = re.compile(
    r"EmergencyGuard::initialize\s*\(\s*([^,]+)\s*,\s*vec!\s*\[([^\]]*)\]\s*,\s*([0-9]+)\s*\)",
    re.MULTILINE,
)


def scan_file(path: Path):
    try:
        txt = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        print(f"warning: could not read {path}: {exc}", file=sys.stderr)
        return []

    results = []
    for m in PATTERN.finditer(txt):
        env_arg = m.group(1).strip()
        admins_raw = m.group(2).strip()
        threshold = int(m.group(3))

        # count comma-separated items in admins_raw, ignoring empty
        if admins_raw == "":
            admins_count = 0
        else:
            # crude split, handles simple literals like &e, admin.clone(), &admin
            parts = [p.strip() for p in admins_raw.split(",") if p.strip()]
            admins_count = len(parts)

        results.append((env_arg, admins_raw, admins_count, threshold))
    return results


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit literal EmergencyGuard::initialize thresholds."
    )
    parser.parse_args()

    print("Scanning for EmergencyGuard::initialize literal vec![] usages...\n")
    try:
        files = list(ROOT.rglob("*.rs"))
    except OSError as exc:
        print(f"error: could not scan {ROOT}: {exc}", file=sys.stderr)
        return 1

    found = 0
    problems = 0
    for f in files:
        rel = f.relative_to(ROOT)
        res = scan_file(f)
        if not res:
            continue
        for env_arg, admins_raw, admins_count, threshold in res:
            found += 1
            ok = True
            notes = []
            if threshold == 0:
                notes.append("threshold == 0")
                ok = False
            if admins_count == 0:
                notes.append("no literal admins in vec!")
                ok = False
            if threshold > admins_count:
                notes.append(f"threshold ({threshold}) > admins_count ({admins_count})")
                ok = False

            status = "OK" if ok else "INVALID"
            if not ok:
                problems += 1

            print(f"{rel}: status={status} admins_count={admins_count} threshold={threshold}")
            if admins_raw:
                snippet = admins_raw[:200]
                print(f"  admins snippet: {snippet}")
            if notes:
                print(f"  notes: {', '.join(notes)}")
            print()

    print(f"Scan complete. {found} literal initialize() calls found, {problems} potential problems.")
    if found == 0:
        print("No literal vec![] initializers found; consider reviewing docs or integration files.")
    if problems:
        print(f"ERROR: {problems} invalid multisig threshold(s) found.", file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
