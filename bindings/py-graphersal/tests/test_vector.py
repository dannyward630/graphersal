"""Vectors: `graphersal.Vector` from lists and float buffers (numpy without a numpy dependency),
read back from properties, passed as parameters, quantized, and coerced by a schema."""

import array

import pytest

import graphersal
from graphersal import Vector


@pytest.fixture
def graph():
    return graphersal.Graph()


def test_construction_and_accessors():
    v = Vector([0.5, -1.0, 0.25])
    assert v.element == "f32"
    assert v.dimension == 3
    assert len(v) == 3
    assert v.tolist() == [0.5, -1.0, 0.25]
    assert Vector([0.1]).tolist() == [0.1]
    assert repr(v) == "Vector([0.5, -1.0, 0.25], element='f32')"
    assert v == Vector((0.5, -1, 0.25))
    assert hash(v) == hash(Vector([0.5, -1.0, 0.25]))
    assert v != Vector([0.5, -1.0, 0.25], element="f16")
    bit = Vector([0.5, -1.0, 0.25], element="bit")
    assert bit.element == "bit"
    assert bit.tolist() == [1.0, -1.0, 1.0]
    i8 = v.quantize("i8")
    assert i8.element == "i8"
    assert abs(i8.tolist()[0] - 0.5) < 1 / 127
    assert i8.quantize("f32").element == "f32"


@pytest.mark.parametrize(
    "values, message",
    [
        ([], "1 to 65536"),
        ([1.0, float("nan")], "NaN"),
        ([float("inf")], "finite"),
        ([1, "x"], "not a number"),
        ([True], "bool"),
        ("abc", "list of numbers"),
        (42, "list of numbers"),
    ],
)
def test_bad_vectors_raise_value_error(values, message):
    with pytest.raises(ValueError, match=message):
        Vector(values)


def test_unknown_element_and_f16_overflow():
    with pytest.raises(ValueError, match="unknown vector element"):
        Vector([1.0], element="f64")
    with pytest.raises(ValueError, match="beyond the range"):
        Vector([1e10], element="f16")


def test_float_buffers_without_numpy():
    assert Vector(array.array("f", [0.5, 2.0])).tolist() == [0.5, 2.0]
    assert Vector(array.array("d", [0.5, 2.0])).tolist() == [0.5, 2.0]
    assert Vector(memoryview(array.array("f", [1.5]))).tolist() == [1.5]
    with pytest.raises(ValueError, match="float32 or float64"):
        Vector(array.array("i", [1, 2]))


def test_numpy_arrays():
    np = pytest.importorskip("numpy")
    emb = np.array([0.5, -1.0, 0.25], dtype=np.float32)
    assert Vector(emb).tolist() == [0.5, -1.0, 0.25]
    assert Vector(emb.astype(np.float64), element="i8").element == "i8"
    matrix = np.ones((2, 4), dtype=np.float32)
    assert Vector(matrix[1]).dimension == 4
    with pytest.raises(ValueError, match="one-dimensional"):
        Vector(matrix)
    back = np.array(Vector(emb).tolist(), dtype=np.float32)
    assert (back == emb).all()


def test_a_vector_round_trips_through_a_property(graph):
    q = Vector([0.1, 0.9])
    graph.execute('g.add_v("doc").property("name", "a").property("emb", q).iterate()', params={"q": q})
    got = graph.query('g.v().values("emb").next()')
    assert isinstance(got, Vector)
    assert got == q
    assert graph.query('g.v().has("emb", q).values("name").next()', params={"q": q}) == "a"
    vertex = graph.query("g.v().next()")
    assert vertex["emb"] == q
    assert graph.query('vector([0.5, 1], "i8")').element == "i8"
    assert graph.query('g.inject(x).quantize("bit").next()', params={"x": q}).element == "bit"


def test_a_declared_vector_coerces_a_list_and_quantizes(graph):
    graph.set_schema(
        {
            "mode": "open",
            "vertices": {
                "doc": {
                    "schema": {
                        "type": "object",
                        "properties": {
                            "emb": {"type": "array", "format": "vector", "element": "i8", "dimension": 3}
                        },
                    }
                }
            },
        }
    )
    graph.execute('g.add_v("doc").property("emb", [0.5, -1.0, 0.25]).iterate()')
    graph.execute('g.add_v("doc").property("emb", q).iterate()', params={"q": Vector([1.0, 2.0, 3.0])})
    got = graph.query('g.v().values("emb").to_list()')
    assert [v.element for v in got] == ["i8", "i8"]
    with pytest.raises(graphersal.GraphersalError, match="vector<i8, 3>"):
        graph.execute('g.add_v("doc").property("emb", [1.0, 2.0]).iterate()')


def test_a_saved_query_takes_a_vector_argument(graph):
    graph.execute('g.add_v("doc").property("name", "a").property("emb", vector([0.5, 0.5])).iterate()')
    graph.define_query(
        {
            "name": "same",
            "body": 'fn same(q) { g.v().has("emb", q).values("name") }',
            "params": {"q": {"schema": {"type": "array", "format": "vector", "element": "f32", "dimension": 2}}},
        }
    )
    assert graph.call_query("same", {"q": Vector([0.5, 0.5])}) == ["a"]
