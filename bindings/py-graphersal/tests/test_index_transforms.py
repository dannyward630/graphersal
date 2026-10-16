"""Property index transforms (book page "Property Indexes", section "Case-insensitive search"):
the `transform` argument of `Graph.create_index`, the `transform` of `Graph.indexes()` and the
query form `has(key, __.to_lower().is(..))`."""

import pytest

import graphersal


def test_a_to_lower_index_is_created_listed_and_serves_the_lowered_query():
    graph = graphersal.Graph.tinkerpop_modern()
    graph.create_index("person", "name", transform="to_lower")
    graph.create_index("person", "name")
    indexes = {index["name"]: index for index in graph.indexes()}
    assert indexes["person_name_to_lower"]["transform"] == "to_lower"
    assert indexes["person_name_to_lower"]["skipped"] == 0
    assert indexes["person_name"]["transform"] is None
    found = graph.execute(
        'g.v().has_label("person").has("name", __.to_lower().is("josh")).values("name").to_list()'
    )
    assert found == ["josh"]


def test_an_unknown_transform_and_a_non_string_key_are_refused():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.create_index("person", "name", transform="upper")
    assert "'upper' is not an index transform this version knows" in str(exc_info.value)
    graph.set_schema(
        {
            "mode": "open",
            "vertices": {
                "person": {
                    "schema": {"type": "object", "properties": {"age": {"type": "integer"}}}
                }
            },
        }
    )
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.create_index("person", "age", transform="to_lower")
    assert "with the transform 'to_lower'" in str(exc_info.value)
    assert graph.indexes() == []
