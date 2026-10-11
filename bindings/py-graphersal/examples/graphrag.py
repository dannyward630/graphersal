"""GraphRAG showcase: a tiny knowledge graph -> the 2-hop neighbourhood of a question entity ->
LLM-ready context lines (JSONL). Only the `graphersal` binding is needed.

    python examples/graphrag.py ["Marie Curie"]
"""

import json
import sys

import graphersal

ENTITIES = [  # (name, type, text): what an extraction pass over documents would yield
    ("Marie Curie", "person", "Physicist and chemist, pioneer of radioactivity research."),
    ("Pierre Curie", "person", "Physicist, co-discoverer of polonium and radium."),
    ("Radium", "element", "Radioactive element discovered in 1898."),
    ("Polonium", "element", "Radioactive element named after Poland."),
    ("Nobel Prize in Physics 1903", "award", "Shared by Becquerel and the Curies."),
    ("University of Paris", "institution", "Where Marie Curie taught from 1906."),
    ("Poland", "country", "Marie Curie's country of birth."),
    ("Paris", "city", "Capital of France."),
]
RELATIONS = [  # (source, relation, target)
    ("Marie Curie", "married_to", "Pierre Curie"),
    ("Marie Curie", "worked_at", "University of Paris"),
    ("Pierre Curie", "discovered", "Radium"),
    ("Pierre Curie", "discovered", "Polonium"),
    ("Pierre Curie", "received", "Nobel Prize in Physics 1903"),
    ("Polonium", "named_after", "Poland"),
    ("University of Paris", "located_in", "Paris"),
]


def build_graph():
    graph = graphersal.Graph()
    for name, kind, text in ENTITIES:  # params are data, never spliced into the script
        graph.execute('g.addV(kind).property("name", name).property("text", text).next()',
                      params={"kind": kind, "name": name, "text": text})
    for src, rel, dst in RELATIONS:
        graph.execute('g.V().has("name", src).as("a").V().has("name", dst)'
                      '.addE(rel).from("a").next()', params={"src": src, "rel": rel, "dst": dst})
    return graph


def context(graph, question_entity, hops=2):
    """One dict per path of exactly `hops` out-steps: the reached entity plus the chain."""
    paths = graph.query('g.V().has("name", q).repeat(__.out()).times(hops)'
                        '.path().by(__.elementMap("name", "text")).to_list()',
                        params={"q": question_entity, "hops": hops},
                        policy=graphersal.AccessPolicy.read_only())  # retrieval only reads
    return [{"entity": p[-1]["name"], "type": p[-1]["label"], "text": p[-1]["text"],
             "path": [step["name"] for step in p], "hops": hops} for p in paths]


if __name__ == "__main__":
    for record in context(build_graph(), sys.argv[1] if len(sys.argv) > 1 else "Marie Curie"):
        print(json.dumps(record, ensure_ascii=False))
