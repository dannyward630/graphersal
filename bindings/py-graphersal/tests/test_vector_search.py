"""Exact vector search from Python: `nearest()`, `similarity()` and `vector_mean()` with the query
vector as a `graphersal.Vector`, a float buffer or a numpy array parameter, and a saved query."""

import array

import pytest

import graphersal
from graphersal import Vector


@pytest.fixture
def graph():
    g = graphersal.Graph()
    for name, emb in [("a", [1.0, 0.0]), ("b", [0.6, 0.8]), ("c", [0.0, 1.0])]:
        g.execute(
            'g.add_v("doc").property("name", n).property("emb", e).iterate()',
            params={"n": name, "e": Vector(emb)},
        )
    return g


def test_nearest_with_a_vector_parameter(graph):
    q = Vector([1.0, 0.1])
    assert graph.query('g.v().nearest("emb", q, 2).values("name").to_list()', params={"q": q}) == ["a", "b"]
    got = graph.query('g.v().nearest("emb", q, 1, #{metric: "l2"}).values("name").next()', params={"q": Vector([0.0, 0.9])})
    assert got == "c"


def test_a_float_buffer_parameter_is_a_vector():
    g = graphersal.Graph()
    assert g.query("type_of(q)", params={"q": array.array("f", [0.5, 1.0])}) == "Vector"
    assert g.query("q.element", params={"q": array.array("d", [0.5, 1.0])}) == "f32"
    # A buffer of another format is no vector.
    with pytest.raises(graphersal.GraphersalError, match="unsupported parameter type"):
        g.query("q", params={"q": b"\x00\x01"})
    with pytest.raises(graphersal.GraphersalError, match="unsupported parameter type"):
        g.query("q", params={"q": array.array("i", [1, 2])})


def test_numpy_query_vectors(graph):
    np = pytest.importorskip("numpy")
    q = np.array([0.0, 1.0], dtype=np.float32)
    assert graph.query('g.v().nearest("emb", q, 2).values("name").to_list()', params={"q": q}) == ["c", "b"]
    scores = graph.query('g.v().similarity("emb", q).to_list()', params={"q": q.astype(np.float64)})
    assert scores == pytest.approx([0.0, 0.8, 1.0])
    with pytest.raises(graphersal.GraphersalError, match="one-dimensional"):
        graph.query("q", params={"q": np.ones((2, 2), dtype=np.float32)})


def test_vector_mean_and_a_saved_search(graph):
    mean = graph.query('g.v().values("emb").vector_mean().next()')
    assert isinstance(mean, Vector)
    assert mean.tolist() == pytest.approx([1.6 / 3, 1.8 / 3])
    graph.define_query(
        {
            "name": "semantic_search",
            "params": {
                "q": {"schema": {"type": "array", "format": "vector", "dimension": 2}},
                "k": {"schema": {"type": "integer"}, "default": 2},
            },
            "body": 'fn semantic_search(q, k) { g.v().nearest("emb", q, k).values("name") }',
        }
    )
    assert graph.call_query("semantic_search", {"q": Vector([0.0, 1.0]), "k": 1}) == ["c"]
    with pytest.raises(graphersal.GraphersalError, match="dimensions 2 and 3"):
        graph.query('g.v().nearest("emb", q, 1).to_list()', params={"q": Vector([1.0, 0.0, 0.0])})
