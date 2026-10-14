#!/usr/bin/env python3
"""Compare review variants sequentially using the existing result/CPU checks."""

import diagnose_current
from pathlib import Path
import json
import os
import hashlib

for case in ["matching-wide", "large-wide"]:
    diagnose_current.CASES[case] = (
        Path(__file__).resolve().parent / "investigation" / f"review-{case}.json", "resource")

for name in ["early", "matching", "subset", "shared", "shared+retain",
             "early+matching", "early+shared", "early+shared+retain",
             "reuse", "early+reuse", "components", "early+components",
             "early+reuse+components", "early+reuse+matching",
             "early+reuse+shared", "early+reuse+shared+retain", "wide",
             "shared+retain+dense", "early+reuse+shared+retain+dense",
             "reuse+shared+retain+dense", "reuse+matching", "shared+retain+bottom",
             "early+reuse+components+matching", "early+reuse+components+shared+retain+bottom",
             "early+reuse+components+matching+shared+retain+bottom",
             "early+reuse+components+wide", "early+reuse+prepare",
             "early+reuse+prepare+matching+shared+retain+bottom", "before", "target10", "target12"]:
    diagnose_current.VARIANTS[name] = {"RE_PROFILE_VARIANT": name}

# GNU time captures peak RSS for each measured child, rather than the cumulative
# maximum in RUSAGE_CHILDREN. Its tiny CPU cost is included in every variant.
run = diagnose_current.subprocess.run


def with_memory(command, **kwargs):
    if kwargs["env"].get("RE_PROFILE_VARIANT") == "before":
        command = [os.environ["RE_REVIEW_BEFORE_BINARY"], *command[1:]]
    usage = Path(command[-1]).with_suffix(".usage.json")
    rss_file = usage.with_suffix(".rss")
    result = run(["/usr/bin/time", "-f", "%M", "-o", str(rss_file), *command], **kwargs)
    usage.write_text(json.dumps({"peak_rss_kib": int(rss_file.read_text().strip()),
        "binary": command[0], "binary_sha256": hashlib.sha256(Path(command[0]).read_bytes()).hexdigest()}))
    rss_file.unlink()
    return result


diagnose_current.subprocess.run = with_memory

if __name__ == "__main__":
    diagnose_current.main()
