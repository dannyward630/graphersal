"""Units of atomicity from Python: the per-traversal default, `atomic=True` (the whole script is
one unit) and `Graph.dry_run` (one unit that is always rolled back, returning its change set).
"""

import json
import uuid

import pytest

import graphersal

U1 = uuid.UUID("550e8400-e29b-41d4-a716-446655440000")

# The first statement adds a vertex, the second one fails (sum() over strings).
ADD_THEN_FAIL = 'g.addV("person").property("name", "ann").next(); g.V().values("name").sum().next()'


def vertex_count(graph):
    return graph.query("g.V().count().next()")


# --- atomic= --------------------------------------------------------------------------------


def test_default_keeps_the_statements_before_a_failing_one():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError):
        graph.execute(ADD_THEN_FAIL)
    # Only the failing traversal was rolled back; the first statement stays.
    assert vertex_count(graph) == 7


def test_atomic_rolls_back_the_whole_script():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.execute(ADD_THEN_FAIL, atomic=True)
    assert vertex_count(graph) == 6
    # The error is the plain step failure, with its help text.
    assert type(exc_info.value) is graphersal.GraphersalError
    assert exc_info.value.help is not None


def test_atomic_commits_a_successful_script():
    graph = graphersal.Graph()
    result = graph.execute(
        'g.addV("person").property("name", "ann").next();'
        'g.addV("person").property("name", "bob").next();'
        'g.V().values("name").order().to_list()',
        atomic=True,
    )
    assert result == ["ann", "bob"]
    assert vertex_count(graph) == 2


def test_atomic_script_can_handle_a_failing_traversal_itself():
    # A traversal inside an atomic script is a savepoint: a failure the script catches rolls
    # back only that traversal.
    graph = graphersal.Graph()
    result = graph.execute(
        'g.addV("a").next();'
        'try { g.addV("b").property("name", "x").V().values("name").sum().next(); } catch (err) { }'
        'g.V().label().to_list()',
        atomic=True,
    )
    assert result == ["a"]
    assert graph.query("g.V().label().to_list()") == ["a"]


def test_atomic_works_with_params_and_on_query():
    graph = graphersal.Graph()
    graph.query('g.addV("n").property("u", params.u).next()', params={"u": U1}, atomic=True)
    assert graph.query('g.V().values("u").next()') == U1


def test_atomic_rejects_a_returned_closure_and_rolls_back():
    graph = graphersal.Graph()
    with pytest.raises(graphersal.ReturnedClosureError) as exc_info:
        graph.execute('g.addV("x").next(); |x| x + 1', atomic=True)
    assert isinstance(exc_info.value, graphersal.GraphersalError)
    assert exc_info.value.help is not None and "closure" in exc_info.value.help
    assert vertex_count(graph) == 0


# --- dry_run --------------------------------------------------------------------------------


def test_dry_run_returns_the_result_and_the_changes_and_leaves_the_graph_untouched():
    graph = graphersal.Graph.tinkerpop_modern()
    run = graph.dry_run(
        'g.V().has("name", "marko").property("age", 30).next();'
        'g.V().hasLabel("software").drop().to_list();'
        "g.V().count().next()"
    )
    assert isinstance(run, graphersal.DryRunResult)
    assert run.result == 4
    changes = run.changes
    assert changes["commit_seq"] == 0
    ops = [m["op"] for m in changes["mutations"]]
    assert ops[0] == "set_property"
    assert ops.count("drop_vertex") == 2
    # Incident edges are explicit and precede their vertex.
    assert ops.count("drop_edge") == 4
    first = changes["mutations"][0]
    assert first["element"] == {"kind": "vertex", "id": "1"}
    assert (first["key"], first["before"], first["after"]) == ("age", 29, 30)
    # Plain data: it serializes as JSON as it is.
    json.dumps(changes)

    assert vertex_count(graph) == 6
    assert graph.query("g.E().count().next()") == 6
    assert graph.query('g.V().has("name", "marko").values("age").next()') == 29


def test_dry_run_result_is_immutable_and_has_a_readable_repr():
    graph = graphersal.Graph()
    run = graph.dry_run('g.addV("x").next(); "done"')
    assert repr(run) == "DryRunResult(result='done', changes=<1 mutation>)"
    assert repr(graph.dry_run("1")) == "DryRunResult(result=1, changes=<0 mutations>)"
    with pytest.raises(AttributeError):
        run.result = 2
    with pytest.raises(AttributeError):
        run.other = 2
    # Not a tuple: it does not unpack.
    with pytest.raises(TypeError):
        _result, _changes = run


def test_dry_run_of_a_read_only_script_has_no_mutations():
    graph = graphersal.Graph.tinkerpop_modern()
    run = graph.dry_run('g.V().hasLabel("person").count().next()')
    assert run.result == 4
    # The modern graph's numeric ids moved the auto-id sequences to 6 (vertices 1-6) and 5
    # (edges 0-5).
    assert run.changes == {
        "commit_seq": 0,
        "committed_at": 0,
        "node_sequence": 6,
        "edge_sequence": 5,
        "mutations": [],
    }


def test_dry_run_changes_carry_the_auto_id_sequences_and_no_commit_time():
    graph = graphersal.Graph()
    first = graph.dry_run('g.addV("x").addV("y").to_list()').changes
    assert (first["commit_seq"], first["committed_at"]) == (0, 0)
    assert (first["node_sequence"], first["edge_sequence"]) == (2, 0)
    # The ids a dry run handed out are never handed out again.
    second = graph.dry_run('g.addV("x").id().next()')
    assert second.result == "3"
    assert second.changes["node_sequence"] == 3


def test_dry_run_tags_uuid_values():
    graph = graphersal.Graph()
    graph.execute('g.addV("n").property("id", "a").property("u", params.u).next()', params={"u": U1})
    changes = graph.dry_run(
        'g.addV("n").property("u", params.u).property("name", "x").next();'
        'g.V().has("id", "a").property("u", params.v).next()',
        params={"u": U1, "v": uuid.UUID(int=1)},
    ).changes
    add, set_property = changes["mutations"]
    assert add["op"] == "add_vertex"
    assert add["labels"] == ["n"]
    tag = {"@type": "g:UUID", "@value": str(U1)}
    assert add["properties"] == {"u": tag, "name": "x"}
    # Property order is kept as stored.
    assert list(add["properties"]) == ["u", "name"]
    assert set_property["before"] == tag
    assert set_property["after"] == {"@type": "g:UUID", "@value": str(uuid.UUID(int=1))}
    # The graph still holds a single vertex with the original uuid.
    assert graph.query('g.V().values("u").to_list()') == [U1]


def test_failing_dry_run_raises_and_leaves_nothing():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError):
        graph.dry_run(ADD_THEN_FAIL)
    assert vertex_count(graph) == 6


def test_dry_run_respects_the_policy():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.PermissionDeniedError):
        graph.dry_run('g.addV("x").next()', policy=graphersal.AccessPolicy.read_only())
    run = graph.dry_run('g.V().count().next()', policy=graphersal.AccessPolicy.read_only())
    assert (run.result, run.changes["mutations"]) == (6, [])


def test_dry_run_rejects_a_returned_closure():
    graph = graphersal.Graph()
    with pytest.raises(graphersal.ReturnedClosureError) as exc_info:
        graph.dry_run('g.addV("x").next(); [1, #{f: |x| x}]')
    assert "result[1].f" in str(exc_info.value)
    assert exc_info.value.help is not None and "closure" in exc_info.value.help
    assert vertex_count(graph) == 0


def test_graph_is_fully_usable_after_a_dry_run():
    graph = graphersal.Graph()
    graph.dry_run('g.addV("x").next()')
    graph.execute('g.addV("y").next()')
    assert graph.query("g.V().label().to_list()") == ["y"]


# --- a traversal returned without a terminal --------------------------------------------------


def test_a_returned_traversal_runs_like_to_list():
    graph = graphersal.Graph.tinkerpop_modern()
    assert graph.execute('g.V().hasLabel("software").values("name").order()') == ["lop", "ripple"]
    # A mutation without a terminal runs too.
    assert graph.execute('g.addV("person").property("name", "ann")')[0]["name"] == "ann"
    assert vertex_count(graph) == 7
    # A bare traversal source yields an empty list.
    assert graph.execute("g") == []


def test_a_returned_traversal_inside_arrays_and_maps_runs_too():
    graph = graphersal.Graph.tinkerpop_modern()
    result = graph.execute('[1, g.V("1").values("age"), #{n: [g.V().count()]}]')
    assert result == [1, [29], {"n": [[6]]}]


def test_a_returned_traversal_runs_after_the_commit_with_atomic():
    graph = graphersal.Graph()
    assert graph.execute('g.addV("a").next(); g.V().label()', atomic=True) == ["a"]


def test_a_returned_traversal_runs_inside_the_dry_run():
    graph = graphersal.Graph()
    run = graph.dry_run('g.addV("a").next(); #{labels: g.V().label()}')
    # It sees the previewed vertex, though the graph never keeps it.
    assert run.result == {"labels": ["a"]}
    assert vertex_count(graph) == 0


def test_a_failing_returned_traversal_raises_without_a_rust_type_path():
    graph = graphersal.Graph.tinkerpop_modern()
    with pytest.raises(graphersal.GraphersalError) as exc_info:
        graph.execute('g.V().values("name").sum()')
    assert "ScriptGraphTraversalSource" not in str(exc_info.value)
    assert "graphersal::" not in str(exc_info.value)
