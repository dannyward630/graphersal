//! Harness for running the vendored Apache TinkerPop Gherkin feature suite against Graphersal.
//!
//! Each submodule owns one concern; later steps of the feature-suite series add their modules here.

// Many model fields and helpers are consumed only by later harness steps (notation, translation,
// execution, comparison, baseline); silence dead-code warnings until they are wired in.
#![allow(dead_code)]

pub mod baseline;
pub mod classify;
pub mod compare;
pub mod dataset;
pub mod execute;
pub mod gherkin;
pub mod inventory;
pub mod notation;
pub mod page;
pub mod report;
pub mod scope;
pub mod storage;
pub mod translate;
