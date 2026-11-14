#!/usr/bin/env python3
"""LLM: Reproduce the path-coloring query through the normal stateless server API."""

import argparse
import json
from pathlib import Path
import time
import urllib.request


def request(url, body):
    """LLM: POST one JSON operation, returning its packet after the computation finishes."""
    data = json.dumps(body, separators=(",", ":")).encode()
    with urllib.request.urlopen(urllib.request.Request(
        url, data=data, headers={"Content-Type": "application/json"}
    ), timeout=120) as response:
        return json.load(response)


def main():
    """LLM: Parse, apply RE four times, then time a fresh fixed-palette streaming query."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:8080/api")
    parser.add_argument("--save-request", type=Path)
    args = parser.parse_args()
    fixture = json.loads((Path(__file__).parent / "definition.json").read_text())
    response = request(args.url, {"type": "parse_problem", "data": fixture})["response"]
    assert response["type"] == "problem", response
    problem = response["data"]

    # LLM: Each transformed snapshot starts with an empty cache, as a new UI card does.
    for _ in range(4):
        response = request(args.url, {"type": "compute", "data": {
            "state": {"problem": problem}, "operation": {"type": "apply_re"}
        }})["response"]
        assert response["type"] == "problem", response
        problem = response["data"]
    body = {"type": "compute", "data": {"state": {"problem": problem},
        "operation": {"type": "check_coloring", "data": {
            "kind": "vertex", "search": {"kind": "fixed", "colors": 25},
            "passive_mode": "general"
        }}}}
    if args.save_request:
        args.save_request.write_text(json.dumps(body, separators=(",", ":")))

    # LLM: Consume the same event stream as the UI. Time only the coloring request;
    # LLM: parsing and the four RE transformations are outside this measurement.
    started = time.perf_counter()
    events = 0
    final = None
    with urllib.request.urlopen(urllib.request.Request(
        args.url + "/stream", data=json.dumps(body, separators=(",", ":")).encode(),
        headers={"Content-Type": "application/json"}
    ), timeout=120) as stream:
        for line in stream:
            if line.startswith(b"data:"):
                event = json.loads(line[5:])
                events += 1
                if event["type"] == "response":
                    final = event["data"]["response"]
    seconds = time.perf_counter() - started
    assert final and final["type"] == "coloring", final
    assert final["data"]["status"] == "solvable", final
    print(json.dumps({"transport": "native_stream", "colors": 25,
                      "status": final["data"]["status"], "seconds": seconds,
                      "events": events}))


if __name__ == "__main__":
    main()
