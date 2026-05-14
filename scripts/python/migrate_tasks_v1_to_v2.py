#!/usr/bin/env python3
"""Regseq compiler harness -- tasks.json schema v1 -> v2 migration.

Step 2 of the harness refactor plan v4. Migrates `.harness/tasks.json` from
schema v1 (top level: just `{"groups": [...]}`; per-task fields: id, name,
spec, status, depends_on, artifacts, fail_reason) to schema v2:

- Top level gains `"schema_version": 2`.
- Every task gains explicit values (no silent synthesis):
  - `phase`: `null`.
  - `spec_refs`: `[]`.
  - `spec_version`: `1`.
  - `history`: one `migrated_in_place` entry timestamped to the spec file's
    mtime (ISO-Z) if the file exists, otherwise to the migration runtime
    with a `"note": "spec file missing at migration time"` field.

Idempotent: refuses to run if `schema_version == 2` is already set.

Atomic write: tempfile + os.replace pattern.
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
TASKS_FILE = REPO_ROOT / ".harness" / "tasks.json"


def iso_z(ts: float) -> str:
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def now_iso_z() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def migrate(data: dict, runtime_ts: str) -> dict:
    if data.get("schema_version") == 2:
        raise SystemExit("error: tasks.json already at schema_version 2; refusing to re-migrate")

    new = {"schema_version": 2, "groups": data["groups"]}
    for group in new["groups"]:
        for task in group["tasks"]:
            spec_rel = task.get("spec")
            if spec_rel:
                spec_path = REPO_ROOT / spec_rel
                if spec_path.exists():
                    ts = iso_z(spec_path.stat().st_mtime)
                    history_entry = {
                        "ts": ts,
                        "event": "migrated_in_place",
                        "spec_version": 1,
                    }
                else:
                    history_entry = {
                        "ts": runtime_ts,
                        "event": "migrated_in_place",
                        "spec_version": 1,
                        "note": "spec file missing at migration time",
                    }
            else:
                history_entry = {
                    "ts": runtime_ts,
                    "event": "migrated_in_place",
                    "spec_version": 1,
                    "note": "no spec path at migration time",
                }

            task["phase"] = None
            task["spec_refs"] = []
            task["spec_version"] = 1
            task["history"] = [history_entry]
    return new


def write_atomic(data: dict) -> None:
    tmp_fd, tmp_path = tempfile.mkstemp(dir=TASKS_FILE.parent, suffix=".tmp", prefix="tasks-")
    try:
        with os.fdopen(tmp_fd, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        os.replace(tmp_path, TASKS_FILE)
    except BaseException:
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        raise


def main() -> None:
    if not TASKS_FILE.exists():
        print(f"error: {TASKS_FILE} not found", file=sys.stderr)
        sys.exit(1)

    with open(TASKS_FILE) as f:
        data = json.load(f)

    runtime_ts = now_iso_z()
    new_data = migrate(data, runtime_ts)
    write_atomic(new_data)

    total = sum(len(g["tasks"]) for g in new_data["groups"])
    print(f"migrated {total} tasks across {len(new_data['groups'])} groups to schema_version 2")


if __name__ == "__main__":
    main()
