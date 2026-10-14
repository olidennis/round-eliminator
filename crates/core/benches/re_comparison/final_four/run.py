#!/usr/bin/env python3
"""Controlled wall/CPU/RSS and complete-output checks for the four experiments."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import compare_review

for variant in ["flow", "fixed", "dynamic", "pipe5", "pipe8", "compressed",
                "grain512", "grain1024", "flow+fixed", "flow+grain512",
                "fixed+grain512", "fixed+grain1024", "flow+fixed+grain512",
                "pipe5+buffered", "pipe8+buffered", "unitgrain", "fixed+unitgrain",
                "dynamic+compressed", "fixed+compressed"]:
    compare_review.diagnose_current.VARIANTS[variant] = {"RE_PROFILE_VARIANT": variant}
for case in ["wide232", "wide522", "sparse174", "sparse522"]:
    compare_review.diagnose_current.CASES[case] = (
        Path(__file__).resolve().parent.parent / "investigation" / f"four-{case}.json", "resource")

if __name__ == "__main__": compare_review.diagnose_current.main()
