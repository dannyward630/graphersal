"""Engine semantics as seen from Python: errors, parameters, rollback of a failing mutating
query, value conversion (unicode, nested containers) and uuid values.
"""

import uuid

import pytest

import graphersal

U1 = uuid.UUID("550e8400-e29b-41d4-a716-446655440000")


# --- errors ---------------------------------------------------------------------------------


def test_step_failure_raises_with_location_and_help():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.execute('g.V().values("name").sum().next()')
    err = exc_info.value
    assert "Step #2 'sum()' execution failed" in str(err)
    # The help text is split off the message, not repeated inside it.
    assert "Help:" not in str(err)
    assert err.help is not None and "[number] is required" in err.help


def test_unknown_step_raises_graphersal_error():
    graph = graphersal.Graph()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.execute("g.V().no_such_step()")
    assert "no_such_step" in str(exc_info.value)


def test_graphersal_error_is_an_exception_subclass():
    assert issubclass(graphersal.GraphersalError, Exception)


def test_missing_graphml_file_raises_graphersal_error(tmp_path):
    with pytest.raises(graphersal.GraphersalError):
        graphersal.Graph.from_graphml(str(tmp_path / "missing.graphml"))


def test_dsl_execute_never_raises_and_carries_the_error():
    graph = graphersal.Graph.tinkerpop_modern()
    run = graph.execute('g.V().values("name").sum().execute()')
    assert run["results"] == []
    assert run["error"]["step_id"] == "2"


# --- parameters -----------------------------------------------------------------------------


def test_params_are_bound_as_variables_and_as_params_map():
    graph = graphersal.Graph.tinkerpop_modern()
    script = 'g.V().has("name", {}).values("age").next()'
    assert graph.query(script.format("who"), params={"who": "josh"}) == 32
    assert graph.query(script.format("params.who"), params={"who": "josh"}) == 32


def test_params_are_data_never_script_text():
    graph = graphersal.Graph.tinkerpop_modern()
    hostile = '") ; g.V().drop().iterate(); ("'
    assert graph.query('g.V().has("name", params.n).to_list()', params={"n": hostile}) == []
    assert graph.query("g.V().count().next()") == 6


def test_bool_param_stays_bool_not_int():
    graph = graphersal.Graph()
    assert graph.execute("params.b", params={"b": False}) is False


def test_int_param_outside_int64_is_rejected():
    graph = graphersal.Graph()
    with pytest.raises(graphersal.GraphersalError, match="int64"):
        graph.execute("params.n", params={"n": 2**70})
    assert graph.execute("params.n", params={"n": 2**63 - 1}) == 2**63 - 1


def test_dict_param_is_rejected_naming_the_key():
    graph = graphersal.Graph()
    with pytest.raises(graphersal.GraphersalError, match=r"params\['d'\]"):
        graph.execute("params.d", params={"d": {"a": 1}})


def test_non_string_param_key_raises_type_error():
    graph = graphersal.Graph()
    with pytest.raises(TypeError):
        graph.execute("1", params={1: 2})


# --- rollback of a failing mutating query ----------------------------------------------------


def snapshot(graph):
    return (
        graph.query("g.V().count().next()"),
        graph.query("g.E().count().next()"),
        graph.query('g.V().order().by(T.id).valueMap().to_list()'),
    )


@pytest.mark.parametrize(
    "script",
    [
        # add_v, then a failing step later in the same traversal
        'g.addV("x").property("name", "n1").V().values("name").sum().next()',
        # a property update, then a failure
        'g.V().has("name", "marko").property("age", 99).V().values("name").sum().next()',
        # an added edge, then a failure
        'g.V().has("name", "marko").as("a").V().has("name", "peter")'
        '.addE("likes").from("a").V().values("name").sum().next()',
    ],
)
def test_failing_mutating_query_leaves_graph_unchanged(script):
    graph = graphersal.Graph.tinkerpop_modern()
    before = snapshot(graph)
    with pytest.raises(graphersal.GraphersalError):
        graph.execute(script)
    assert snapshot(graph) == before


def test_succeeding_mutating_query_is_kept():
    graph = graphersal.Graph.tinkerpop_modern()
    graph.execute('g.V().has("name", "marko").property("age", 30).next()')
    assert graph.query('g.V().has("name", "marko").values("age").next()') == 30


# --- value conversion -----------------------------------------------------------------------


def test_unicode_round_trips_through_labels_keys_and_values():
    graph = graphersal.Graph()
    graph.execute(
        'g.addV(params.label).property(params.key, params.value).next()',
        params={"label": "Stadt", "key": "naïve 名前", "value": "Zürich 日本語 ✓ 🚀"},
    )
    row = graph.query('g.V().hasLabel("Stadt").elementMap().next()')
    assert row["label"] == "Stadt"
    assert row["naïve 名前"] == "Zürich 日本語 ✓ 🚀"


def test_nested_maps_and_lists_convert_to_dicts_and_lists():
    graph = graphersal.Graph()
    value = graph.execute('#{a: [1, #{b: "ç", c: [true, ()]}], "ключ": 2.5, e: [[], #{}]}')
    assert value == {"a": [1, {"b": "ç", "c": [True, None]}], "ключ": 2.5, "e": [[], {}]}
    assert type(value["a"][0]) is int and type(value["ключ"]) is float


def test_group_count_and_path_results_are_native_containers():
    graph = graphersal.Graph.tinkerpop_modern()
    counts = graph.query("g.V().groupCount().by(T.label).next()")
    assert counts == {"person": 4, "software": 2}
    paths = graph.query(
        'g.V().has("name", "marko").out("knows").out("created").path().by("name").to_list()'
    )
    assert sorted(paths) == [["marko", "josh", "lop"], ["marko", "josh", "ripple"]]


# --- uuid -----------------------------------------------------------------------------------


def test_uuid_param_is_stored_and_read_back_as_uuid():
    graph = graphersal.Graph()
    graph.execute('g.addV("doc").property("key", params.u).next()', params={"u": U1})
    stored = graph.query('g.V().hasLabel("doc").values("key").next()')
    assert isinstance(stored, uuid.UUID) and stored == U1
    assert graph.query('g.V().has("key", params.u).count().next()', params={"u": U1}) == 1


def test_uuid_param_does_not_equal_its_string_form():
    graph = graphersal.Graph()
    graph.execute('g.addV("doc").property("key", params.u).next()', params={"u": U1})
    assert graph.query('g.V().has("key", params.s).count().next()', params={"s": str(U1)}) == 0


def test_uuid_literal_in_script_converts_to_uuid():
    graph = graphersal.Graph()
    assert graph.execute(f'UUID.fromString("{U1}")') == U1
