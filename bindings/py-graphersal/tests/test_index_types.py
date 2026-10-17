"""Property index types (book page "Property Indexes", section "Index types"): the `type`
argument of `Graph.create_index`, the `type` of `Graph.indexes()` and the create-time check
against the schema."""

import pytest

import graphersal


def test_the_type_defaults_to_memory_ordered_and_is_listed():
    graph = graphersal.Graph.tinkerpop_modern()
    graph.create_index("person", "age")
    graph.create_index("software", "lang", type="memory-ordered")
    indexes = {index["name"]: index for index in graph.indexes()}
    assert indexes["person_age"]["type"] == "memory-ordered"
    assert indexes["software_lang"]["type"] == "memory-ordered"
    assert indexes["software_lang"]["served"] is True
    assert "reason" not in indexes["software_lang"]


def test_an_unknown_type_is_refused_with_help():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.create_index("person", "age", type="hash")
    assert "'hash' is not an index type this version knows" in str(exc_info.value)
    assert "memory-ordered" in exc_info.value.help
    assert graph.indexes() == []


def test_a_key_the_schema_declares_as_a_vector_or_array_is_refused():
    graph = graphersal.Graph()
    graph.set_schema(
        {
            "mode": "open",
            "vertices": {
                "doc": {
                    "schema": {
                        "type": "object",
                        "properties": {
                            "emb": {"type": "array", "format": "vector", "element": "f32"},
                            "tags": {"type": "array", "items": {"type": "string"}},
                            "title": {"type": ["string", "null"]},
                        },
                    }
                }
            },
        }
    )
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.create_index("doc", "emb")
    assert "vector<f32>" in str(exc_info.value)
    assert "nearest()" in exc_info.value.help
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.create_index("doc", "tags")
    assert "array<string>" in str(exc_info.value)
    graph.create_index("doc", "title")
    assert [index["key"] for index in graph.indexes()] == ["title"]
