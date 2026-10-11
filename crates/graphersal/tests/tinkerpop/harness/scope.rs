//! The compatibility scope: scenarios that are excluded from the suite, in two families.
//!
//! 1. **Strategies** ([`EXCLUSIONS`]): every scenario that calls `withStrategies(...)` /
//!    `withoutStrategies(...)`, detected from the Gremlin text (below).
//! 2. **Deliberate incompatibilities** ([`INCOMPATIBILITIES`]): an explicit, reviewed table of
//!    scenario ids grouped by reason, where Graphersal differs from TinkerPop on purpose (a user
//!    decision, documented in `book/src/for_developers/tinkerpop_deviations.md`). Ids only, never
//!    tags or content, so a scenario added by a suite upgrade is never excluded silently. A
//!    strategy exclusion wins over an incompatibility ([`scope_exclusion`]).
//!
//! An excluded scenario is not executed, gets the outcome class [`Class::Excluded`] and is left out
//! of every total and percentage (headline, per-category and per-file tables), and out of the
//! baseline. A baseline entry for an excluded scenario is stale, like one for a nonexistent id.
//!
//! A scenario is excluded when its traversal, its graph initializer or any of its `GraphCount`
//! traversals calls `withStrategies(...)` / `withoutStrategies(...)` — with *any* strategy, as a
//! bare class reference (`withoutStrategies(ReadOnlyStrategy)`) or as an instance
//! (`withStrategies(new SubgraphStrategy(vertices: ...))`). [`EXCLUSIONS`] gives the reason of
//! each strategy the vendored suite names; any other `*Strategy` (catch-all) gets
//! [`REASON_STRATEGY_CONFIG`]. The feature file a scenario lives in is irrelevant; a strategy that
//! is only *named* elsewhere (tag, scenario name, string literal outside the call) does not exclude
//! it, and `withSideEffect`/`withSack`/`with(key, value)` stay in scope.
//!
//! [`Class::Excluded`]: super::classify::Class::Excluded

use super::gherkin::{Assertion, Scenario};

/// Reason for the access-control strategies.
pub const REASON_ACCESS_CONTROL: &str = "views/partitions/read-only/row-level security are provided by \
     Graphersal's own access-control mechanism, not by TinkerPop strategies";

/// Reason for the GraphComputer strategies.
pub const REASON_OLAP: &str = "GraphComputer/OLAP — not applicable to Graphersal";

/// Reason for every other strategy (optimization, decoration, verification, ...); also the
/// catch-all for a strategy name not listed in [`EXCLUSIONS`].
pub const REASON_STRATEGY_CONFIG: &str = "TinkerPop strategy configuration is not supported — \
     Graphersal will provide its own unified, hierarchical execution-metadata mechanism \
     (replacing with()/withStrategies()/parameters)";

/// Strategy name recorded for a `withStrategies(...)` call without any `*Strategy` token.
pub const UNNAMED_STRATEGY: &str = "(unnamed strategy)";

/// One excluded strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exclusion {
    /// The strategy's simple class name.
    pub strategy: &'static str,
    /// Why its scenarios are out of scope.
    pub reason: &'static str,
}

/// Every strategy whose scenarios are out of the compatibility scope.
///
/// The OLAP entries are all `org.apache.tinkerpop.gremlin.process.computer.traversal.strategy.*`
/// classes in TinkerPop 3.8.2, i.e. they only act on a `GraphComputer` traversal.
pub const EXCLUSIONS: &[Exclusion] = &[
    Exclusion {
        strategy: "SubgraphStrategy",
        reason: REASON_ACCESS_CONTROL,
    },
    Exclusion {
        strategy: "PartitionStrategy",
        reason: REASON_ACCESS_CONTROL,
    },
    Exclusion {
        strategy: "ReadOnlyStrategy",
        reason: REASON_ACCESS_CONTROL,
    },
    Exclusion {
        strategy: "VertexProgramStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "VertexProgramRestrictionStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "ComputerVerificationStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "ComputerFinalizationStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "HaltedTraverserStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "MessagePassingReductionStrategy",
        reason: REASON_OLAP,
    },
    Exclusion {
        strategy: "GraphFilterStrategy",
        reason: REASON_OLAP,
    },
    config("AdjacentToIncidentStrategy"),
    config("ByModulatorOptimizationStrategy"),
    config("ConnectiveStrategy"),
    config("CountStrategy"),
    config("EarlyLimitStrategy"),
    config("EdgeLabelVerificationStrategy"),
    config("ElementIdStrategy"),
    config("FilterRankingStrategy"),
    config("IdentityRemovalStrategy"),
    config("IncidentToAdjacentStrategy"),
    config("InlineFilterStrategy"),
    config("LambdaRestrictionStrategy"),
    config("LazyBarrierStrategy"),
    config("MatchAlgorithmStrategy"),
    config("MatchPredicateStrategy"),
    config("OptionsStrategy"),
    config("OrderLimitStrategy"),
    config("PathProcessorStrategy"),
    config("PathRetractionStrategy"),
    config("ProductiveByStrategy"),
    config("ProfileStrategy"),
    config("ReferenceElementStrategy"),
    config("RepeatUnrollStrategy"),
    config("ReservedKeysVerificationStrategy"),
    config("SeedStrategy"),
    config("StandardVerificationStrategy"),
];

const fn config(strategy: &'static str) -> Exclusion {
    Exclusion {
        strategy,
        reason: REASON_STRATEGY_CONFIG,
    }
}

/// Which family of the compatibility scope excludes a scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionKind {
    /// The scenario uses `withStrategies(...)` / `withoutStrategies(...)`.
    Strategy,
    /// The scenario is listed in [`INCOMPATIBILITIES`].
    Incompatibility,
}

/// Why a scenario is out of scope: the strategy (or incompatibility code) that excludes it and
/// its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excluded {
    /// Which family excludes the scenario.
    pub kind: ExclusionKind,
    /// The strategy's simple class name (or [`UNNAMED_STRATEGY`]); for an incompatibility its
    /// [`Incompatibility::code`].
    pub strategy: String,
    /// Why it is out of scope; for an incompatibility its [`Incompatibility::title`].
    pub reason: &'static str,
}

/// Prefix of `Outcome.detail` for a scenario excluded as a deliberate incompatibility (a strategy
/// exclusion keeps the bare strategy name as its detail).
pub const INCOMPAT_DETAIL_PREFIX: &str = "incompat:";

/// One reason for which Graphersal deliberately differs from TinkerPop, with the scenarios that
/// are therefore out of the compatibility scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Incompatibility {
    /// Stable code (kebab-case).
    pub code: &'static str,
    /// Short title.
    pub title: &'static str,
    /// One paragraph: why Graphersal differs.
    pub why: &'static str,
    /// Book page (and optional `#anchor`), relative to `book/src`, that explains it.
    pub doc: &'static str,
    /// Scenario ids (`<feature file>::<scenario name>`).
    pub ids: &'static [&'static str],
}

/// Every deliberate incompatibility. To move a scenario back into scope, remove its id here, run
/// the suite and add the baseline entry it now needs (`TINKERPOP_UPDATE_BASELINE=1`), then fix
/// the docs. The guard tests in `tinkerpop_features.rs` fail when an id does not exist or when a
/// listed scenario starts to pass.
pub const INCOMPATIBILITIES: &[Incompatibility] = &[
    Incompatibility {
        code: "select-undeclared-label",
        title: "select() of a label that is never declared",
        why: "select(\"a\"), select(\"a\", \"b\") and select(Pop.x, ..) on a step label that no as() or side-effect key of the query declares raise Graphersal's own detailed diagnostic, which names the label and shows how to declare it, before execution; TinkerPop silently filters the traverser out. A silent filter hides typos.",
        doc: "for_developers/tinkerpop_deviations.md#deliberate-differences",
        ids: &[
            "map/Select.feature::g_V_selectXaX",
            "map/Select.feature::g_V_selectXaX_count",
            "map/Select.feature::g_V_selectXa_bX",
            "map/Select.feature::g_V_selectXfirst_aX",
            "map/Select.feature::g_V_selectXfirst_a_bX",
            "map/Select.feature::g_V_selectXlast_aX",
            "map/Select.feature::g_V_selectXlast_a_bX",
            "map/Select.feature::g_V_selectXall_aX",
            "map/Select.feature::g_V_selectXall_a_bX",
        ],
    },
    Incompatibility {
        code: "property-element-order",
        title: "order() over property elements",
        why: "order() over properties() elements: TinkerPop orders Property elements by their own total order (key, then value, with cross-type rules) and by property id; properties() here yields value-bearing handles that are ordered by value only, and Graphersal has no property ids. Property elements are handles, not values.",
        doc: "users_guide/property_elements.md#deviations",
        ids: &[
            "semantics/Orderability.feature::g_E_properties_order_byXdescX_value",
            "semantics/Orderability.feature::g_E_properties_order_value",
            "semantics/Orderability.feature::g_V_out_out_properties_asXheadX_path_order_byXascX_selectXheadX_value",
            "semantics/Orderability.feature::g_V_out_out_properties_asXheadX_path_order_byXdescX_selectXheadX_value",
            "semantics/Orderability.feature::g_V_properties_order_id",
        ],
    },
    Incompatibility {
        code: "multi-meta-properties",
        title: "Multi-properties and meta-properties",
        why: "The storage model is one value per property key, by design, with no properties on properties: Cardinality.list and Cardinality.set fail with UnsupportedCardinality and meta-property arguments are not stored. Every non-passing scenario tagged @MultiProperties or @MetaProperties that runs here (the executable part of the reason) is listed. The scenarios that need the crew dataset (the multi-/meta-property dataset, which the GraphSON import loads with one value per key: several entries fold into an array and meta-properties into _meta) are listed too.",
        doc: "users_guide/upserts.md#not-supported-yet",
        ids: &[
            "filter/Drop.feature::g_V_properties_propertiesXstartTimeX_drop",
            "map/AddVertex.feature::g_V_addVXanimalX_propertyXname_valuesXnameXX_propertyXname_an_animalX_propertyXvaluesXnameX_labelX",
            "map/AddVertex.feature::g_V_hasXname_markoX_propertyXfriendWeight_outEXknowsX_weight_sum__acl_privateX",
            "map/AddVertex.feature::g_addVXpersonX_propertyXname_markoX_propertyXfriendWeight_null_acl_nullX",
            "map/AddVertex.feature::g_addVXpersonX_propertyXsingle_name_stephenX_propertyXsingle_name_stephenm_since_2010X",
            "map/AddVertex.feature::g_addV_propertyXname_marko_since_2010X_withXkey_valueX_propertiesXnameX_valuesXsince_keyX",
            "map/AddVertex.feature::get_g_addVXpersonX_propertyXsingle_name_stephenX_propertyXsingle_name_stephenm_since_2010X",
            "map/AddVertex.feature::g_V_hasXname_fooX_propertyXname_setXbarX_age_43X",
            "map/AddVertex.feature::g_V_hasXname_fooX_propertyXset_name_bar_age_singleX43XX",
            "map/AddVertex.feature::g_addVXanimalX_propertyXname_mateoX_propertyXname_gateoX_propertyXname_cateoX_propertyXage_5X",
            "map/MergeVertex.feature::g_injectX0X_mergeVXlabel_person_name_markoX_propertyXname_vadas_acl_publicX",
            "map/MergeVertex.feature::g_mergeVXlabel_person_name_markoX_propertyXname_vadas_acl_publicX",
            "map/MergeVertex.feature::g_mergeVXlabel_person_name_stephenX_propertyXlist_name_steveX",
            "map/MergeVertex.feature::g_mergeVXname_aliceX_optionXonCreate_age_81_label_person_setX",
            "map/MergeVertex.feature::g_mergeVXname_aliceX_optionXonCreate_age_81_setX",
            "map/MergeVertex.feature::g_mergeVXname_aliceX_optionXonCreate_age_setX81XX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_age_33_singleX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_age_listX33XX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_age_setX31XX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_age_setX33XX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_age_singleX33XX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_name_allen_age_setX31X_singleX",
            "map/MergeVertex.feature::g_mergeVXname_markoX_optionXonMatch_name_allen_age_singleX31X_singleX",
            "map/MergeVertex.feature::g_withSideEffectXc_label_person_name_markoX_withSideEffectXm_age_19X_mergeVXselectXcXX_optionXonMatch_sideEffectXpropertiesXageX_dropX_selectXmXX_option",
            "map/MergeVertex.feature::g_withSideEffectXm_age_19X_V_hasXperson_name_markoX_mergeVXselectXcXX_optionXonMatch_sideEffectXpropertiesXageX_dropX_selectXmXX_option",
            // The crew dataset (multi- and meta-properties).
            "branch/Local.feature::g_V_localXpropertiesXlocationX_order_byXvalueX_limitX2XX_value",
            "branch/Repeat.feature::g_VX1X_repeatXrepeatXunionXout_uses_out_traversesXX_whereXloops_isX0X_timesX1X_timeX2X_name",
            "filter/Dedup.feature::g_V_both_properties_properties_dedup_count",
            "filter/Has.feature::g_V_hasXlocationX",
            "filter/HasLabel.feature::g_E_hasLabelXuses_traversesX",
            "map/Combine.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_combineXseattle_vancouverX_orderXlocalX",
            "map/Conjoin.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_orderXlocalX_conjoinX1X",
            "map/Difference.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_differenceXseattle_vancouverX",
            "map/Disjunct.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_disjunctXseattle_vancouverX",
            "map/Element.feature::g_VXv7_properties_properties_element_element",
            "map/Element.feature::g_V_properties_properties_element_element",
            "map/Intersect.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_intersectXseattle_vancouverX",
            "map/Merge.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_mergeXseattle_vancouverX",
            "map/Product.feature::g_V_valueMapXlocationX_selectXvaluesX_unfold_productXdulles_seattle_vancouverX_unfold",
            "map/Select.feature::g_V_hasXname_gremlinX_inEXusesX_order_byXskill_ascX_asXaX_outV_asXbX_selectXa_bX_byXskillX_byXnameX",
            "map/ValueMap.feature::g_VX1X_valueMapXname_locationX_byXunfoldX_by",
        ],
    },
    Incompatibility {
        code: "null-property-removal",
        title: "property(k, null) removes the property",
        why: "Scenarios tagged @DisallowNullPropertyValues: TinkerPop removes the property on property(k, null); Graphersal stores a real null, and removal is the explicit remove_property() step. By design: no hidden deletes.",
        doc: "users_guide/removing_properties.md",
        ids: &[
            "map/AddEdge.feature::g_V_outE_propertyXweight_nullX",
            "map/AddVertex.feature::g_V_hasLabelXpersonX_propertyXname_nullX",
            "map/MergeEdge.feature::g_mergeEXlabel_knows_out_marko_in_vadasX_optionXonMatch_weight_nullX",
        ],
    },
    Incompatibility {
        code: "map-to-string",
        title: "asString() of a map",
        why: "valueMap().asString() yields Java Map.toString text ({name=[marko]}) in TinkerPop; Graphersal never produces Java formats as data text (the physical codec is the only text form), so a map is a cast error.",
        doc: "users_guide/property_elements.md#deviations",
        ids: &[
            "map/AsString.feature::g_V_valueMapXnameX_asString",
            "map/AsString.feature::g_V_valueMapXnameX_order_fold_asStringXlocalX",
        ],
    },
    Incompatibility {
        code: "path-retraction-artefact",
        title: "expected result depends on TinkerPop's PathRetractionStrategy",
        why: "expected result depends on TinkerPop's PathRetractionStrategy dropping repeat-loop labels; Graphersal returns the full shortest-path set (verified on TinkerPop: 24 with the strategy, 30 without)",
        doc: "for_developers/tinkerpop_deviations.md#deliberate-differences",
        ids: &["integrated/Paths.feature::g_V_shortestpath"],
    },
    Incompatibility {
        code: "match-step",
        title: "match() pattern matching",
        why: "The declarative match() step is not planned: declarative pattern matching will come through a Cypher/GQL front end on top of the same engine instead. match is also a reserved Rhai word.",
        doc: "for_developers/tinkerpop_deviations.md#excluded-for-good",
        ids: &[
            "branch/Local.feature::g_V_localXmatchXproject__created_person__person_name_nameX_selectXname_projectX_by_byXnameX",
            "map/Match.feature::g_V_asXaX_out_asXbX_matchXa_out_count_c__b_in_count_cX",
            "map/Match.feature::g_V_asXaX_out_asXbX_matchXa_out_count_c__orXa_knows_b__b_in_count_c__and__c_isXgtX2XXXX",
            "map/Match.feature::g_V_hasLabelXsongsX_matchXa_name_b__a_performances_cX_selectXb_cX_count",
            "map/Match.feature::g_V_matchXa_0sungBy_b__a_0sungBy_c__b_writtenBy_d__c_writtenBy_e__d_hasXname_George_HarisonX__e_hasXname_Bob_MarleyXX",
            "map/Match.feature::g_V_matchXa_0sungBy_b__a_0writtenBy_c__b_writtenBy_dX_whereXc_sungBy_dX_whereXd_hasXname_GarciaXX",
            "map/Match.feature::g_V_matchXa_0sungBy_b__a_0writtenBy_c__b_writtenBy_d__c_sungBy_d__d_hasXname_GarciaXX",
            "map/Match.feature::g_V_matchXa__a_both_b__b_both_cX_dedupXa_bX",
            "map/Match.feature::g_V_matchXa__a_out_b__notXa_created_bXX",
            "map/Match.feature::g_V_matchXa_both_b__b_both_cX_dedupXa_bX_byXlabelX",
            "map/Match.feature::g_V_matchXa_created_b__a_repeatXoutX_timesX2XX_selectXa_bX",
            "map/Match.feature::g_V_matchXa_created_b__b_0created_aX",
            "map/Match.feature::g_V_matchXa_created_b__b_0created_cX_whereXa_neq_cX_selectXa_cX",
            "map/Match.feature::g_V_matchXa_created_lop_b__b_0created_29_cX_whereXc_repeatXoutX_timesX2XX_selectXa_b_cX",
            "map/Match.feature::g_V_matchXa_created_lop_b__b_0created_29_c__c_whereXrepeatXoutX_timesX2XXX",
            "map/Match.feature::g_V_matchXa_followedBy_count_isXgtX10XX_b__a_0followedBy_count_isXgtX10XX_bX_count",
            "map/Match.feature::g_V_matchXa_hasXname_GarciaX__a_0writtenBy_b__a_0sungBy_bX",
            "map/Match.feature::g_V_matchXa_hasXname_GarciaX__a_0writtenBy_b__b_followedBy_c__c_writtenBy_d__whereXd_neqXaXXX",
            "map/Match.feature::g_V_matchXa_hasXsong_name_sunshineX__a_mapX0followedBy_weight_meanX_b__a_0followedBy_c__c_filterXweight_whereXgteXbXXX_outV_dX_selectXdX_byXnameX",
            "map/Match.feature::g_V_matchXa_knows_b__a_created_cX",
            "map/Match.feature::g_V_matchXa_knows_b__andXa_created_c__b_created_c__andXb_created_count_d__a_knows_count_dXXX",
            "map/Match.feature::g_V_matchXa_knows_b__b_created_cX",
            "map/Match.feature::g_V_matchXa_knows_b__b_created_c__a_created_cX_dedupXa_b_cX_selectXaX_byXnameX",
            "map/Match.feature::g_V_matchXa_knows_b__b_created_lop__b_matchXb_created_d__d_0created_cX_selectXcX_cX_selectXa_b_cX",
            "map/Match.feature::g_V_matchXa_knows_count_bX_selectXbX",
            "map/Match.feature::g_V_matchXa_outEXcreatedX_order_byXweight_descX_limitX1X_inV_b__b_hasXlang_javaXX_selectXa_bX_byXnameX",
            "map/Match.feature::g_V_matchXa_outXknowsX_name_bX_identity",
            "map/Match.feature::g_V_matchXa_out_bX",
            "map/Match.feature::g_V_matchXa_out_bX_selectXb_idX",
            "map/Match.feature::g_V_matchXa_whereXa_neqXcXX__a_created_b__orXa_knows_vadas__a_0knows_and_a_hasXlabel_personXX__b_0created_c__b_0created_count_isXgtX1XXX_selectXa_b_cX_byXidX",
            "map/Match.feature::g_V_matchXb_created_c__a_knows_bX",
            "map/Match.feature::g_V_matchXd_0knows_a__d_hasXname_vadasX__a_knows_b__b_created_cX",
            "map/Match.feature::g_V_matchXwhereXandXa_created_b__b_0created_count_isXeqX3XXXX__a_both_b__whereXb_inXX",
            "map/Match.feature::g_V_notXmatchXa_age_b__a_name_cX_whereXb_eqXcXX_selectXaXX_name",
            "map/Match.feature::g_V_out_out_matchXa_0created_b__b_0knows_cX_selectXcX_outXcreatedX_name",
            "map/Match.feature::g_V_valueMap_matchXa_selectXnameX_bX",
        ],
    },
    Incompatibility {
        code: "extra-number-types",
        title: "Byte, short, BigInteger, BigDecimal, char and binary types",
        why: "The number model is int64 and float64 by design: GType.BYTE, SHORT, BIGINT, BIGDECIMAL, CHAR and BINARY and BigInteger literals have no Graphersal value type and are not planned; the width literals 1b/1s/1n/1m collapse to int64/float64 in the harness.",
        doc: "for_developers/tinkerpop_deviations.md#excluded-for-good",
        ids: &[
            "data/BigDecimal.feature::g_V_valuesXageX_isXtypeOfXGType_BIGDECIMALXX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_isXgtX0XX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_mathXaddX0_5XX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_maxX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_minX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_project_byXidentityX_byXmathXmulX10XXX",
            "data/BigDecimal.feature::g_V_valuesXintX_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_sumX",
            "data/BigDecimal.feature::g_injectX99X_asNumberXGType_BIGDECIMALX_isXtypeOfXGType_BIGDECIMALXX_groupCount",
            "data/BigInt.feature::g_V_valuesXageX_isXtypeOfXGType_BIGINTXX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_isXeqX42XX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_mathXmulX1000XX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_maxX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_minX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_project_byXidentityX_byXmathXaddX999XXX",
            "data/BigInt.feature::g_V_valuesXintX_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_sumX",
            "data/BigInt.feature::g_injectX777X_asNumberXGType_BIGINTX_isXtypeOfXGType_BIGINTXX_groupCount",
            "data/Byte.feature::g_V_valuesXageX_isXtypeOfXGType_BYTEXX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_chooseXisXeqX12XX_constantXtwelveX_constantXotherXX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_isXltX10XX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_mathXaddX20XX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_project_byXidentityX_byXmathXmulX2XXX",
            "data/Byte.feature::g_V_valuesXintX_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_sumX",
            "data/Byte.feature::g_injectX15X_asNumberXGType_BYTEX_isXtypeOfXGType_BYTEXX_groupCount",
            "data/Short.feature::g_V_valuesXageX_isXtypeOfXGType_SHORTXX",
            "data/Short.feature::g_V_valuesXintX_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX",
            "data/Short.feature::g_V_valuesXintX_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX_isXbetweenX20_30XX",
            "data/Short.feature::g_V_valuesXintX_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX_mathXmulX10XX",
            "data/Short.feature::g_V_valuesXintX_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX_maxX",
            "data/Short.feature::g_V_valuesXintX_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX_minX",
            "data/Short.feature::g_injectX42X_asNumberXGType_SHORTX_isXtypeOfXGType_SHORTXX_storeXaX_capXaX",
            "filter/TypeOf.feature::g_V_valuesXageX_isXtypeOfXGType_BINARYXX",
            "filter/TypeOf.feature::g_V_valuesXageX_isXtypeOfXGType_CHARXX",
            "map/AsNumber.feature::g_injectX12X_asNumberXGType_BYTEX",
            "map/AsNumber.feature::g_injectX1_000X_asNumberXGType_BIGINTX",
            "map/AsNumber.feature::g_injectX1_2_3_4_0x5X_asNumber_sum_asNumberXGType_BYTEX",
            "map/AsNumber.feature::g_injectX300X_asNumberXGType_BYTEX",
            "map/AsNumber.feature::g_injectX32768X_asNumberXGType_SHORTX",
            "map/AsNumber.feature::g_injectX5X_asNumberXGType_BYTEX",
            "sideEffect/Sack.feature::g_withSackXBigInteger_TEN_powX1000X_assignX_V_localXoutXknowsX_barrierXnormSackXX_inXknowsX_barrier_sack",
        ],
    },
    Incompatibility {
        code: "io-step",
        title: "io() / read() / write()",
        why: "The io() source step with read()/write() and the IO tokens is not planned: GraphML import and export is Graphersal's own API (GraphSource::from_graphml, import_graphml, the CLI --graph), not a traversal step.",
        doc: "for_developers/tinkerpop_deviations.md#excluded-for-good",
        ids: &[
            "sideEffect/Read.feature::g_io_readXgraphmlX",
            "sideEffect/Read.feature::g_io_readXgraphsonX",
            "sideEffect/Read.feature::g_io_readXkryoX",
            "sideEffect/Read.feature::g_io_read_withXreader_graphmlX",
            "sideEffect/Read.feature::g_io_read_withXreader_graphsonX",
            "sideEffect/Read.feature::g_io_read_withXreader_gryoX",
            "sideEffect/Write.feature::g_io_writeXgraphmlX",
            "sideEffect/Write.feature::g_io_writeXgraphsonX",
            "sideEffect/Write.feature::g_io_writeXkryoX",
            "sideEffect/Write.feature::g_io_write_withXwriter_graphmlX",
            "sideEffect/Write.feature::g_io_write_withXwriter_graphsonX",
            "sideEffect/Write.feature::g_io_write_withXwriter_gryoX",
        ],
    },
    Incompatibility {
        code: "graph-algorithms",
        title: "Graph algorithm steps (pageRank, shortestPath, connectedComponent, peerPressure)",
        why: "The GraphComputer vertex-program steps pageRank(), shortestPath(), connectedComponent() and peerPressure() (and their token classes) are not planned for the traversal language; these scenarios are also @GraphComputerOnly.",
        doc: "for_developers/tinkerpop_deviations.md#excluded-for-good",
        ids: &[
            "map/ConnectedComponent.feature::g_V_connectedComponent_hasXcomponentX",
            "map/ConnectedComponent.feature::g_V_connectedComponent_withXEDGES_bothEXknowsXX_withXPROPERTY_NAME_clusterX_project_byXnameX_byXclusterX",
            "map/ConnectedComponent.feature::g_V_dedup_connectedComponent_hasXcomponentX",
            "map/ConnectedComponent.feature::g_V_hasLabelXsoftwareX_connectedComponent_project_byXnameX_byXcomponentX",
            "map/PageRank.feature::g_V_hasLabelXpersonX_pageRank_withXpropertyName_kpageRankX_project_byXnameX_byXvaluesXpageRankX_mathX",
            "map/PageRank.feature::g_V_hasLabelXsoftwareX_hasXname_rippleX_pageRankX1X_withXedges_inEXcreatedX_withXtimes_1X_withXpropertyName_priorsX_inXcreatedX_unionXboth__identityX_valueMapXname_priorsX",
            "map/PageRank.feature::g_V_outXcreatedX_groupXmX_byXlabelX_pageRankX1X_withXpropertyName_pageRankX_withXedges_inEX_withXtimes_1X_inXcreatedX_groupXmX_byXpageRankX_capXmX",
            "map/PageRank.feature::g_V_outXcreatedX_pageRank_withXedges_bothEX_withXpropertyName_projectRankX_withXtimes_0X_valueMapXname_projectRankX",
            "map/PageRank.feature::g_V_pageRank_hasXpageRankX",
            "map/PageRank.feature::g_V_pageRank_order_byXpageRank_descX_byXnameX_name",
            "map/PageRank.feature::g_V_pageRank_order_byXpageRank_descX_name_limitX2X",
            "map/PageRank.feature::g_V_pageRank_withXedges_outEXknowsXX_withXpropertyName_friendRankX_project_byXnameX_byXvaluesXfriendRankX_mathX",
            "map/PageRank.feature::g_V_pageRank_withXpropertyName_pageRankX_asXaX_outXknowsX_pageRank_asXbX_selectXa_bX_by_byXmathX",
            "map/PeerPressure.feature::g_V_hasXname_rippleX_inXcreatedX_peerPressure_withXedges_outEX_withyXpropertyName_clusterX_repeatXunionXidentity__bothX_timesX2X_dedup_valueMapXname_clusterX",
            "map/PeerPressure.feature::g_V_peerPressure_hasXclusterX",
            "map/PeerPressure.feature::g_V_peerPressure_withXpropertyName_clusterX_withXedges_outEXknowsXX_pageRankX1X_byXrankX_withXedges_outEXknowsX_withXtimes_2X_group_byXclusterX_byXrank_sumX_limitX100X",
            "map/ShortestPath.feature::g_V_both_dedup_shortestPath",
            "map/ShortestPath.feature::g_V_hasXname_danielX_shortestPath_targetXhasXname_stephenXX_edgesXbothEXusesXX",
            "map/ShortestPath.feature::g_V_hasXname_markoX_shortestPath",
            "map/ShortestPath.feature::g_V_hasXname_markoX_shortestPath_maxDistanceX1X",
            "map/ShortestPath.feature::g_V_hasXname_markoX_shortestPath_targetXhasLabelXsoftwareXX",
            "map/ShortestPath.feature::g_V_hasXname_markoX_shortestPath_targetXhasXname_joshXX_distanceXweightX",
            "map/ShortestPath.feature::g_V_hasXname_vadasX_shortestPath_distanceXweightX_maxDistanceX1_3X",
            "map/ShortestPath.feature::g_V_hasXsong_name_MIGHT_AS_WELLX_shortestPath_targetXhasXsong_name_MAYBE_YOU_KNOW_HOW_I_FEELXX_edgesXoutEXfollowedByXX_distanceXweightX",
            "map/ShortestPath.feature::g_V_shortestPath",
            "map/ShortestPath.feature::g_V_shortestPath_directionXINX",
            "map/ShortestPath.feature::g_V_shortestPath_edgesIncluded",
            "map/ShortestPath.feature::g_V_shortestPath_edgesIncluded_edgesXoutEX",
            "map/ShortestPath.feature::g_V_shortestPath_edgesXoutEX",
            "map/ShortestPath.feature::g_V_shortestPath_targetXhasXname_markoXX",
            "map/ShortestPath.feature::g_V_shortestPath_targetXvaluesXnameX_isXmarkoXX",
        ],
    },
];

/// The incompatibility that lists a scenario id.
pub fn deliberate(id: &str) -> Option<&'static Incompatibility> {
    INCOMPATIBILITIES.iter().find(|i| i.ids.contains(&id))
}

/// The incompatibility with a code.
pub fn incompatibility(code: &str) -> Option<&'static Incompatibility> {
    INCOMPATIBILITIES.iter().find(|i| i.code == code)
}

/// Why `scenario` is out of the compatibility scope, or `None`: a strategy exclusion first, else
/// a deliberate incompatibility.
pub fn scope_exclusion(scenario: &Scenario) -> Option<Excluded> {
    exclusion(scenario).or_else(|| {
        deliberate(&scenario.id).map(|i| Excluded {
            kind: ExclusionKind::Incompatibility,
            strategy: i.code.to_string(),
            reason: i.title,
        })
    })
}

/// The exclusion entry of a strategy name, if it is listed.
pub fn lookup(strategy: &str) -> Option<&'static Exclusion> {
    EXCLUSIONS.iter().find(|e| e.strategy == strategy)
}

/// The reason of a strategy name: its [`EXCLUSIONS`] entry, else the catch-all
/// [`REASON_STRATEGY_CONFIG`].
pub fn reason_of(strategy: &str) -> &'static str {
    lookup(strategy).map_or(REASON_STRATEGY_CONFIG, |e| e.reason)
}

/// Position of a strategy in [`EXCLUSIONS`] (unlisted names sort last).
pub fn rank(strategy: &str) -> usize {
    EXCLUSIONS
        .iter()
        .position(|e| e.strategy == strategy)
        .unwrap_or(EXCLUSIONS.len())
}

/// The class names passed to `withStrategies(...)` / `withoutStrategies(...)` in `text`, in
/// order: every identifier ending in `Strategy` inside the call's parentheses (`new` and
/// constructor arguments are skipped); a call without any such name yields [`UNNAMED_STRATEGY`].
pub fn strategies_in(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for call in ["withStrategies(", "withoutStrategies("] {
        let mut rest = text;
        while let Some(at) = rest.find(call) {
            let args = &rest[at + call.len()..];
            let end = closing_paren(args).unwrap_or(args.len());
            let before = out.len();
            out.extend(
                identifiers(&args[..end]).filter(|id| id.ends_with("Strategy") && id.len() > 8),
            );
            if out.len() == before {
                out.push(UNNAMED_STRATEGY);
            }
            rest = &args[end..];
        }
    }
    out
}

/// Byte offset of the `)` that closes an already-open parenthesis (string literals skipped).
fn closing_paren(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Identifier tokens of `text` (string literals are not skipped; strategy names never occur there
/// in the vendored suite).
fn identifiers(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|t| t.starts_with(|c: char| c.is_ascii_alphabetic()))
}

/// Every Gremlin text of a scenario that is executed: traversal, initializer, `GraphCount`s.
fn texts(scenario: &Scenario) -> impl Iterator<Item = &str> {
    scenario
        .traversal
        .as_deref()
        .into_iter()
        .chain(scenario.initializer.as_deref())
        .chain(scenario.assertions.iter().filter_map(|a| match a {
            Assertion::GraphCount { traversal, .. } => Some(traversal.as_str()),
            _ => None,
        }))
}

/// Why `scenario` is out of scope, or `None` when it is in scope. Among several strategies the most
/// specific reason wins (access control, then OLAP, then configuration); within one reason, the
/// strategy named first in the scenario's text.
pub fn exclusion(scenario: &Scenario) -> Option<Excluded> {
    let reason_rank = |s: &&str| match reason_of(s) {
        REASON_ACCESS_CONTROL => 0,
        REASON_OLAP => 1,
        _ => 2,
    };
    let strategy = texts(scenario)
        .flat_map(strategies_in)
        .min_by_key(reason_rank)?;
    Some(Excluded {
        kind: ExclusionKind::Strategy,
        strategy: strategy.to_string(),
        reason: reason_of(strategy),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::gherkin;

    fn scenario(given: &str, traversal: &str) -> Scenario {
        let text = format!(
            "Feature: T\n\n  Scenario: s\n    Given the empty graph\n{given}    And the traversal of\n      \"\"\"\n      {traversal}\n      \"\"\"\n    When iterated to list\n    Then the result should be empty\n"
        );
        let feature = gherkin::parse_feature("map/T.feature", &text).expect("parses");
        feature.scenarios.into_iter().next().expect("one scenario")
    }

    fn excluded(traversal: &str) -> Option<String> {
        exclusion(&scenario("", traversal)).map(|e| e.strategy)
    }

    #[test]
    fn bare_and_constructed_strategies_match() {
        assert_eq!(
            excluded("g.withStrategies(SubgraphStrategy).V()"),
            Some("SubgraphStrategy".into())
        );
        assert_eq!(
            excluded(
                r#"g.withStrategies(new SubgraphStrategy(vertices: __.has("name", P.within("josh", "lop")))).V()"#
            ),
            Some("SubgraphStrategy".into())
        );
        assert_eq!(
            excluded("g.withoutStrategies(ReadOnlyStrategy).V()"),
            Some("ReadOnlyStrategy".into())
        );
        assert_eq!(
            excluded(
                r#"g.withStrategies(new PartitionStrategy(partitionKey: "_partition", writePartition: "a", readPartitions: ["a"])).V()"#
            ),
            Some("PartitionStrategy".into())
        );
        // A specific reason beats the catch-all.
        assert_eq!(
            excluded("g.withoutStrategies(CountStrategy, VertexProgramStrategy).V()"),
            Some("VertexProgramStrategy".into())
        );
        assert_eq!(
            lookup("HaltedTraverserStrategy").map(|e| e.reason),
            Some(REASON_OLAP)
        );
    }

    #[test]
    fn every_other_strategy_is_excluded_as_configuration() {
        let reason = |t: &str| exclusion(&scenario("", t)).map(|e| e.reason);
        assert_eq!(
            excluded("g.withStrategies(CountStrategy).V().count()"),
            Some("CountStrategy".into())
        );
        assert_eq!(
            reason("g.withoutStrategies(RepeatUnrollStrategy).V()"),
            Some(REASON_STRATEGY_CONFIG)
        );
        // Catch-all: a strategy the table does not know.
        assert_eq!(
            reason("g.withStrategies(new FutureStrategy(x: 1)).V()"),
            Some(REASON_STRATEGY_CONFIG)
        );
        assert_eq!(
            excluded("g.withStrategies(new FutureStrategy(x: 1)).V()"),
            Some("FutureStrategy".into())
        );
        // The listed name wins over a string literal after the call.
        assert_eq!(
            excluded("g.withStrategies(new SeedStrategy(seed: 1)).V().as(\"ReadOnlyStrategy\")"),
            Some("SeedStrategy".into())
        );
    }

    #[test]
    fn merely_named_strategies_and_other_source_steps_do_not_match() {
        assert_eq!(excluded(r#"g.V().has("name", "SubgraphStrategy")"#), None);
        assert_eq!(excluded("g.V()"), None);
        assert_eq!(
            excluded(r#"g.withSideEffect("a", 1).withSack(1.0).V()"#),
            None
        );
        assert_eq!(excluded(r#"g.with("key", "SeedStrategy").V()"#), None);
    }

    #[test]
    fn deliberate_lookup_and_strategy_precedence() {
        let id = "map/AsString.feature::g_V_valueMapXnameX_asString";
        assert_eq!(deliberate(id).map(|i| i.code), Some("map-to-string"));
        assert_eq!(deliberate("map/Call.feature::g_call"), None);
        assert_eq!(
            incompatibility("map-to-string").map(|i| i.ids.len()),
            Some(2)
        );

        let mut sc = scenario("", "g.V()");
        sc.id = id.to_string();
        let e = scope_exclusion(&sc).expect("listed");
        assert_eq!(e.kind, ExclusionKind::Incompatibility);
        assert_eq!(e.strategy, "map-to-string");
        assert_eq!(e.reason, "asString() of a map");

        // A strategy wins over the incompatibility table.
        let mut sc = scenario("", "g.withStrategies(CountStrategy).V()");
        sc.id = id.to_string();
        let e = scope_exclusion(&sc).expect("excluded");
        assert_eq!(e.kind, ExclusionKind::Strategy);
        assert_eq!(e.strategy, "CountStrategy");

        // Neither family: in scope.
        assert_eq!(scope_exclusion(&scenario("", "g.V()")), None);
    }

    #[test]
    fn initializer_only_use_matches() {
        let sc = scenario(
            "    And the graph initializer of\n      \"\"\"\n      g.withStrategies(new PartitionStrategy(partitionKey: \"p\", writePartition: \"a\")).addV(\"x\")\n      \"\"\"\n",
            "g.V()",
        );
        assert_eq!(
            exclusion(&sc).map(|e| e.strategy),
            Some("PartitionStrategy".into())
        );
        assert_eq!(
            exclusion(&sc).map(|e| e.reason),
            Some(REASON_ACCESS_CONTROL)
        );
    }
}
