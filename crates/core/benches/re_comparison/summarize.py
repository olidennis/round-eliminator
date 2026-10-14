#!/usr/bin/env python3
"""Summarize per-step and complete-sequence times without mixing sample counts."""

from collections import defaultdict
import csv
import json
from pathlib import Path
import statistics

HERE = Path(__file__).resolve().parent
rows = [json.loads(line) for line in (HERE / "results.jsonl").read_text().splitlines()]
groups = defaultdict(dict)
for row in rows:
    if row["event"] != "measured":
        continue
    mode = row["mode"]
    if mode.startswith("maximize"):
        comparison = "maximize_same_input" if mode == "maximize" else "maximize_cleaned_input"
        values = [("old", row["old_maximize_ms"]), ("new", row["new_maximize_ms"])]
    else:
        comparison = "old_button" if mode == "old-button" else "re_only"
        values = [("new" if mode == "new-re" else "old", row["milliseconds"])]
    for implementation, value in values:
        groups[(row["case"], comparison, row["step"], implementation)][row["sample"]] = value

# Total medians come from per-run sums, not sums of individual step medians.
totals = defaultdict(lambda: defaultdict(float))
counts = defaultdict(lambda: defaultdict(int))
for (case, comparison, step, implementation), values in groups.items():
    for sample, value in values.items():
        totals[(case, comparison, implementation)][sample] += value
        counts[(case, comparison, implementation)][sample] += 1
for (case, comparison, implementation), values in totals.items():
    expected = 2 if case == "test-all-second" else next(
        row["steps"] for row in rows if row["event"] == "environment")
    completed = {sample: value for sample, value in values.items()
        if counts[(case, comparison, implementation)][sample] == expected}
    if completed:
        groups[(case, comparison, "total", implementation)] = completed

with (HERE / "summary.csv").open("w", newline="") as output:
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(["case", "comparison", "step", "implementation", "samples", "median_ms", "min_ms", "max_ms"])
    for (case, comparison, step, implementation), values in sorted(groups.items(), key=lambda item: str(item[0])):
        samples = list(values.values())
        writer.writerow([case, comparison, step, implementation, len(samples),
            f"{statistics.median(samples):.6f}", f"{min(samples):.6f}", f"{max(samples):.6f}"])
