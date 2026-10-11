//! The Rhai expression-depth limit is the same in every build profile (128 / 64) and safe on a
//! 2 MiB thread stack.
#![cfg(feature = "script")]

use crate::script_host::eval_value;
use graphersal::prelude::*;
use std::sync::Arc;

fn chain(calls: usize) -> String {
    format!("g.V(){}.count().next()", ".out()".repeat(calls))
}

fn nested_unions(levels: usize) -> String {
    let mut inner = "__.out()".to_string();
    for _ in 0..levels {
        inner = format!("__.union({inner})");
    }
    format!("g.V().union({inner}).count().next()")
}

/// Runs `script` on a 2 MiB thread; `Ok(())` when it evaluates, `Err(message)` when it fails.
fn on_small_stack(script: String) -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let graph = Arc::new(GraphSource::tinkerpop_modern());
            eval_value(graph, &script)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .expect("spawn")
        .join()
        .expect("the thread must not overflow its stack")
}

#[test]
fn a_chain_of_one_hundred_calls_evaluates() {
    on_small_stack(chain(100)).expect("100 calls");
}

#[test]
fn thirty_nested_unions_evaluate() {
    on_small_stack(nested_unions(30)).expect("30 levels");
}

#[test]
fn a_chain_of_one_hundred_fifty_calls_is_rejected_cleanly() {
    let err = on_small_stack(chain(150)).expect_err("150 calls exceed the limit");
    assert!(
        err.contains("complexity") || err.contains("Expression exceeds"),
        "{err}"
    );
}

#[test]
fn the_boundary_is_one_depth_regardless_of_profile() {
    let (inside, outside) = (BOUNDARY_INSIDE, BOUNDARY_INSIDE + 1);
    on_small_stack(chain(inside)).unwrap_or_else(|e| panic!("{inside} calls: {e}"));
    let err = on_small_stack(chain(outside)).expect_err("one call more");
    assert!(
        err.contains("complexity") || err.contains("Expression exceeds"),
        "{err}"
    );
}

const BOUNDARY_INSIDE: usize = 123;
