"""Runs the scripts in `examples/` so they cannot rot."""

import json
import pathlib
import subprocess
import sys

EXAMPLES = pathlib.Path(__file__).resolve().parent.parent / "examples"


def run_example(name, *args):
    done = subprocess.run(
        [sys.executable, str(EXAMPLES / name), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=True,
    )
    return done.stdout


def test_graphrag_prints_one_jsonl_context_record_per_two_hop_path():
    lines = run_example("graphrag.py", "Marie Curie").splitlines()
    records = [json.loads(line) for line in lines]
    assert {r["entity"] for r in records} == {
        "Radium",
        "Polonium",
        "Nobel Prize in Physics 1903",
        "Paris",
    }
    for record in records:
        assert set(record) == {"entity", "type", "text", "path", "hops"}
        assert record["hops"] == 2
        assert len(record["path"]) == 3
        assert record["path"][0] == "Marie Curie"
        assert record["path"][-1] == record["entity"]
        assert isinstance(record["text"], str) and record["text"]


def test_graphrag_entity_without_two_hop_neighbours_prints_nothing():
    assert run_example("graphrag.py", "Radium") == ""
