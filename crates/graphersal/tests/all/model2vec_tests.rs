//! The Model2Vec embedder (feature `model2vec`, book page "Embeddings", section "Real models:
//! Model2Vec") on a TINY synthetic model written by the test (a word-level `tokenizer.json`, a
//! hand-written `model.safetensors`, a `config.json`): load, dimension, identity (stable,
//! path-independent, from the manifest), batch order, the `embed` step through the DSL, load
//! failures, odd input without a panic. No network: a real model is used only by the
//! `#[ignore]`d test at the end, from a directory the developer downloaded.

#![cfg(all(feature = "model2vec", feature = "script"))]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use graphersal::prelude::*;
use graphersal::script::rhai::Dynamic;
use graphersal::vector::{
    EmbedContext, EmbedError, Embedder, Embedders, Model2Vec, Vector, VectorElement, VectorMetric,
};

// ---------------------------------------------------------------------------------------------
// A tiny model
// ---------------------------------------------------------------------------------------------

/// The vocabulary of the tiny model, by id.
const VOCAB: [&str; 8] = [
    "[UNK]",
    "graph",
    "traversal",
    "vector",
    "marko",
    "josh",
    "lop",
    "ripple",
];

/// The embedding rows of the tiny model (dimension 4), by token id.
const ROWS: [[f32; 4]; 8] = [
    [9.0, 9.0, 9.0, 9.0],
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.5],
    [0.5, 0.5, 0.0, 0.0],
    [0.0, 0.5, 0.5, 0.0],
    [0.25, 0.0, 0.0, 1.0],
    [0.0, 0.0, 0.75, 0.75],
];

/// A unique, empty scratch directory.
fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("graphersal_m2v_{}_{name}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `tokenizer.json` with a word-level model over `vocab` (lowercased, split on whitespace and
/// punctuation), `[UNK]` the unknown token.
fn tokenizer_json(vocab: &[&str]) -> String {
    let entries: Vec<String> = vocab
        .iter()
        .enumerate()
        .map(|(id, token)| format!("{token:?}: {id}"))
        .collect();
    format!(
        r#"{{"version": "1.0", "truncation": null, "padding": null, "added_tokens": [],
            "normalizer": {{"type": "Lowercase"}}, "pre_tokenizer": {{"type": "Whitespace"}},
            "post_processor": null, "decoder": null,
            "model": {{"type": "WordLevel", "vocab": {{{}}}, "unk_token": "[UNK]"}}}}"#,
        entries.join(", ")
    )
}

/// A safetensors file holding `tensors` (name, dtype, shape, little-endian data).
fn safetensors(tensors: &[(&str, &str, Vec<usize>, Vec<u8>)]) -> Vec<u8> {
    let mut offset = 0;
    let mut entries = Vec::new();
    for (name, dtype, shape, data) in tensors {
        let dims: Vec<String> = shape.iter().map(ToString::to_string).collect();
        entries.push(format!(
            r#""{name}": {{"dtype": "{dtype}", "shape": [{}], "data_offsets": [{offset}, {}]}}"#,
            dims.join(", "),
            offset + data.len()
        ));
        offset += data.len();
    }
    let mut header = format!("{{{}}}", entries.join(", "));
    while header.len() % 8 != 0 {
        header.push(' ');
    }
    let mut out = (header.len() as u64).to_le_bytes().to_vec();
    out.extend_from_slice(header.as_bytes());
    for (_, _, _, data) in tensors {
        out.extend_from_slice(data);
    }
    out
}

fn f32_bytes(rows: &[[f32; 4]]) -> Vec<u8> {
    rows.iter()
        .flatten()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Writes the tiny model into `dir` (with `normalize` in its config when given).
fn write_model(dir: &Path, normalize: Option<bool>) {
    let config = match normalize {
        Some(flag) => format!(r#"{{"model_type": "model2vec", "normalize": {flag}}}"#),
        None => r#"{"model_type": "model2vec"}"#.to_string(),
    };
    std::fs::write(dir.join("config.json"), config).unwrap();
    std::fs::write(dir.join("tokenizer.json"), tokenizer_json(&VOCAB)).unwrap();
    std::fs::write(
        dir.join("model.safetensors"),
        safetensors(&[("embeddings", "F32", vec![8, 4], f32_bytes(&ROWS))]),
    )
    .unwrap();
}

fn tiny_model(name: &str) -> PathBuf {
    let dir = scratch(name);
    write_model(&dir, Some(true));
    dir
}

/// The vector the model must give for the ids: the mean of their rows, L2-normalized (in the
/// order of `model2vec-rs`: f32 sums, division, then the norm).
fn expected(ids: &[usize], normalize: bool) -> Vec<f32> {
    let mut sum = [0.0f32; 4];
    for id in ids {
        for (s, v) in sum.iter_mut().zip(ROWS[*id]) {
            *s += v;
        }
    }
    for s in &mut sum {
        *s /= ids.len() as f32;
    }
    if normalize {
        let norm = sum.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-12);
        for s in &mut sum {
            *s /= norm;
        }
    }
    sum.to_vec()
}

fn embed(model: &Model2Vec, texts: &[&str]) -> Result<Vec<Vector>, EmbedError> {
    model.embed(texts, &EmbedContext::new())
}

fn assert_close(actual: &Vector, expected: &[f32]) {
    let actual = actual.to_f32_vec();
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-6, "{actual:?} != {expected:?}");
    }
}

fn unavailable_reason(result: Result<Model2Vec, EmbedError>) -> String {
    match result {
        Err(EmbedError::Unavailable { reason, .. }) => reason,
        Err(other) => panic!("expected Unavailable, got {other:?}"),
        Ok(model) => panic!("expected a load failure, got {model:?}"),
    }
}

// ---------------------------------------------------------------------------------------------
// Load and info
// ---------------------------------------------------------------------------------------------

#[test]
fn loads_a_model_and_describes_it() {
    let dir = tiny_model("load");
    let model = Model2Vec::load("tiny", &dir).unwrap();
    let info = model.info();
    assert_eq!(info.name, "tiny");
    assert_eq!(info.dimension, 4);
    assert_eq!(info.element, VectorElement::F32);
    assert_eq!(info.metric, VectorMetric::Cosine);
    assert_eq!(info.max_batch, Model2Vec::DEFAULT_MAX_BATCH);
    assert!(!info.deterministic);
    assert!(info.local);
    assert!(model.normalize());
    assert_eq!(model.max_tokens(), Model2Vec::DEFAULT_MAX_TOKENS);
    assert!(
        info.model.starts_with("model2vec@") && info.model.len() == "model2vec@".len() + 12,
        "{}",
        info.model
    );
    assert!(
        info.model["model2vec@".len()..]
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    );

    let options = Model2Vec::options().with_max_tokens(3).with_max_batch(7);
    let model = options.load("tiny", &dir).unwrap();
    assert_eq!(model.info().max_batch, 7);
    assert_eq!(model.max_tokens(), 3);
}

#[test]
fn embeds_the_mean_of_the_token_rows() {
    let dir = tiny_model("mean");
    let model = Model2Vec::load("tiny", &dir).unwrap();
    let vectors = embed(&model, &["Graph", "graph vector", "lop, ripple! marko"]).unwrap();
    assert_close(&vectors[0], &expected(&[1], true));
    assert_close(&vectors[1], &expected(&[1, 3], true));
    assert_close(&vectors[2], &expected(&[6, 7, 4], true));
    // Unknown words are left out of the mean, as in model2vec-rs.
    let with_unknown = embed(&model, &["graph zebra vector"]).unwrap();
    assert_eq!(with_unknown[0], vectors[1]);

    // Without normalization (config or override) the mean stays as it is.
    let raw = Model2Vec::options()
        .with_normalize(false)
        .load("raw", &dir)
        .unwrap();
    assert!(!raw.normalize());
    assert_close(
        &embed(&raw, &["graph vector"]).unwrap()[0],
        &expected(&[1, 3], false),
    );
    let plain = scratch("plain");
    write_model(&plain, Some(false));
    let plain = Model2Vec::load("plain", &plain).unwrap();
    assert!(!plain.normalize());
    assert_close(
        &embed(&plain, &["graph vector"]).unwrap()[0],
        &expected(&[1, 3], false),
    );
}

#[test]
fn max_tokens_cuts_long_texts() {
    let dir = tiny_model("max_tokens");
    let model = Model2Vec::options()
        .with_max_tokens(1)
        .load("tiny", &dir)
        .unwrap();
    let vectors = embed(&model, &["graph vector traversal"]).unwrap();
    assert_close(&vectors[0], &expected(&[1], true));
}

#[test]
fn batches_keep_the_order_of_the_texts() {
    let dir = tiny_model("order");
    let model = Model2Vec::load("tiny", &dir).unwrap();
    let words = [
        "graph",
        "traversal",
        "vector",
        "marko",
        "josh",
        "lop",
        "ripple",
    ];
    // More texts than one internal chunk (64), in a mixed order.
    let texts: Vec<&str> = (0..200).map(|i| words[(i * 5 + 3) % words.len()]).collect();
    let batch = embed(&model, &texts).unwrap();
    assert_eq!(batch.len(), texts.len());
    for (text, vector) in texts.iter().zip(&batch) {
        assert_eq!(&embed(&model, &[text]).unwrap()[0], vector, "{text}");
    }
}

// ---------------------------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------------------------

/// Copies the files of `from` into `to` (flat).
fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

#[test]
fn the_identity_is_stable_and_independent_of_the_path() {
    let dir = tiny_model("identity_a");
    let identity = Model2Vec::load("a", &dir).unwrap().info().model.to_string();
    assert_eq!(Model2Vec::model_identity(&dir).unwrap(), identity);
    assert_eq!(
        Model2Vec::load("other-name", &dir).unwrap().info().model,
        identity,
        "the registry name is not part of the identity"
    );

    let elsewhere = scratch("identity_b").join("nested").join("copy");
    std::fs::create_dir_all(&elsewhere).unwrap();
    copy_dir(&dir, &elsewhere);
    assert_eq!(
        Model2Vec::load("a", &elsewhere).unwrap().info().model,
        identity
    );

    // A changed file is another model.
    let changed = scratch("identity_c");
    copy_dir(&dir, &changed);
    let mut rows = ROWS;
    rows[1][0] = 2.0;
    std::fs::write(
        changed.join("model.safetensors"),
        safetensors(&[("embeddings", "F32", vec![8, 4], f32_bytes(&rows))]),
    )
    .unwrap();
    assert_ne!(
        Model2Vec::load("a", &changed).unwrap().info().model,
        identity
    );

    // A normalization other than the config's is part of it.
    let raw = Model2Vec::options()
        .with_normalize(false)
        .load("a", &dir)
        .unwrap();
    assert_eq!(raw.info().model, format!("{identity}?normalize=false"));
    let same = Model2Vec::options()
        .with_normalize(true)
        .load("a", &dir)
        .unwrap();
    assert_eq!(same.info().model, identity);
}

#[test]
fn the_manifest_names_the_repository_in_the_identity() {
    let dir = tiny_model("manifest");
    let bare = Model2Vec::model_identity(&dir).unwrap();
    let hash = bare.strip_prefix("model2vec@").unwrap().to_string();
    std::fs::write(
        dir.join(Model2Vec::MANIFEST_FILE),
        r#"{"format": 1, "repo": "acme/tiny-model", "revision": "main", "files": []}"#,
    )
    .unwrap();
    let model = Model2Vec::load("tiny", &dir).unwrap();
    assert_eq!(
        model.info().model,
        format!("model2vec/acme/tiny-model@{hash}")
    );
    // The registry accepts it as a model identity.
    let registry = Embedders::builder().with(Arc::new(model)).unwrap().build();
    assert!(
        registry
            .by_model(&format!("model2vec/acme/tiny-model@{hash}"))
            .is_some()
    );

    for bad in [
        r#"{"repo": "../../etc"}"#,
        r#"{"repo": "a b"}"#,
        r#"{"revision": "main"}"#,
        "not json",
    ] {
        std::fs::write(dir.join(Model2Vec::MANIFEST_FILE), bad).unwrap();
        let reason = unavailable_reason(Model2Vec::load("tiny", &dir));
        assert!(reason.contains("manifest"), "{bad}: {reason}");
    }
}

#[test]
fn the_sentence_transformers_layouts_load() {
    let flat = scratch("st_flat");
    write_model(&flat, Some(true));
    let identity = Model2Vec::model_identity(&flat).unwrap();
    std::fs::rename(
        flat.join("config.json"),
        flat.join("config_sentence_transformers.json"),
    )
    .unwrap();
    let model = Model2Vec::load("st", &flat).unwrap();
    assert_eq!(model.info().dimension, 4);
    assert_eq!(
        model.info().model,
        identity,
        "the identity is over the roles, not the names"
    );

    let nested = scratch("st_nested");
    let sub = nested.join("0_StaticEmbedding");
    std::fs::create_dir_all(&sub).unwrap();
    write_model(&sub, Some(true));
    std::fs::rename(
        sub.join("config.json"),
        nested.join("config_sentence_transformers.json"),
    )
    .unwrap();
    assert_eq!(Model2Vec::load("st", &nested).unwrap().info().dimension, 4);
}

// ---------------------------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------------------------

#[test]
fn a_missing_directory_or_file_is_unavailable_with_the_download_hint() {
    let missing = std::env::temp_dir().join("graphersal_m2v_no_such_dir");
    let reason = unavailable_reason(Model2Vec::load("tiny", &missing));
    assert!(reason.contains("does not exist"), "{reason}");
    assert!(
        reason.contains("graphersal embed-model download"),
        "{reason}"
    );

    for file in ["config.json", "tokenizer.json", "model.safetensors"] {
        let dir = tiny_model("missing");
        std::fs::remove_file(dir.join(file)).unwrap();
        let reason = unavailable_reason(Model2Vec::load("tiny", &dir));
        assert!(reason.contains(file), "{file}: {reason}");
        assert!(reason.contains("embed-model download"), "{reason}");
        assert!(Model2Vec::model_identity(&dir).is_err());
    }
    let err = Model2Vec::load("tiny", &missing).unwrap_err();
    assert!(!err.help().is_empty());
}

#[test]
fn broken_files_are_unavailable() {
    let cases: Vec<(&str, &str, Vec<u8>)> = vec![
        ("config.json", "not JSON", b"{".to_vec()),
        ("tokenizer.json", "tokenizer", b"{\"model\": 1}".to_vec()),
        ("model.safetensors", "does not parse", b"garbage".to_vec()),
        (
            "model.safetensors",
            "no embedding tensor",
            safetensors(&[("other", "F32", vec![8, 4], f32_bytes(&ROWS))]),
        ),
        (
            "model.safetensors",
            "not 2-dimensional",
            safetensors(&[("embeddings", "F32", vec![32], f32_bytes(&ROWS))]),
        ),
        (
            "model.safetensors",
            "element type",
            safetensors(&[("embeddings", "F64", vec![8, 2], f32_bytes(&ROWS))]),
        ),
        // Fewer rows than the vocabulary has ids: refused at load, never an out-of-range lookup.
        (
            "model.safetensors",
            "do not belong together",
            safetensors(&[("embeddings", "F32", vec![4, 4], f32_bytes(&ROWS[..4]))]),
        ),
        // A mapping that points outside the matrix.
        (
            "model.safetensors",
            "do not belong together",
            safetensors(&[
                ("embeddings", "F32", vec![8, 4], f32_bytes(&ROWS)),
                (
                    "mapping",
                    "I64",
                    vec![8],
                    [0i64, 1, 2, 3, 4, 5, 6, 99]
                        .iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect(),
                ),
            ]),
        ),
        (
            "model.safetensors",
            "do not belong together",
            safetensors(&[
                ("embeddings", "F32", vec![8, 4], f32_bytes(&ROWS)),
                (
                    "mapping",
                    "I32",
                    vec![8],
                    [0i32, 1, 2, 3, 4, 5, 6, -1]
                        .iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect(),
                ),
            ]),
        ),
    ];
    for (file, needle, bytes) in cases {
        let dir = tiny_model("broken");
        std::fs::write(dir.join(file), bytes).unwrap();
        let reason = unavailable_reason(Model2Vec::load("tiny", &dir));
        assert!(reason.contains(needle), "{file} / {needle}: {reason}");
    }
}

#[test]
fn a_valid_mapping_redirects_the_rows() {
    let dir = tiny_model("mapping");
    // Every token uses the row of "graph" (id 1).
    std::fs::write(
        dir.join("model.safetensors"),
        safetensors(&[
            ("embeddings", "F32", vec![8, 4], f32_bytes(&ROWS)),
            (
                "mapping",
                "I64",
                vec![8],
                [0i64, 1, 1, 1, 1, 1, 1, 1]
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect(),
            ),
        ]),
    )
    .unwrap();
    let model = Model2Vec::load("tiny", &dir).unwrap();
    let vectors = embed(&model, &["ripple vector"]).unwrap();
    assert_close(&vectors[0], &expected(&[1], true));
}

#[test]
fn odd_input_never_panics() {
    let dir = tiny_model("odd");
    let model = Model2Vec::load("tiny", &dir).unwrap();
    for (text, index) in [("", 0), ("   ", 0), ("zebra quokka", 0), ("\u{0}\u{1}", 0)] {
        match embed(&model, &["graph", text]) {
            Err(EmbedError::NoFeatures { index: at, .. }) => assert_eq!(at, index + 1, "{text:?}"),
            other => panic!("{text:?}: {other:?}"),
        }
    }
    let long = "graph ".repeat(100_000);
    let odd = [
        "\u{feff}graph\u{200b}",
        "GRAPH\tVECTOR\r\n",
        "😀 graph 😀",
        "ﬁ ǅ İ graph",
        long.as_str(),
    ];
    let vectors = embed(&model, &odd).unwrap();
    assert_eq!(vectors.len(), odd.len());
    // Cut to 512 tokens: still the row of "graph".
    assert_close(&vectors[4], &expected(&[1], true));
}

#[test]
fn a_cancelled_context_stops_between_chunks() {
    let dir = tiny_model("cancel");
    let model = Model2Vec::load("tiny", &dir).unwrap();
    let token = graphersal::exec::CancelToken::new();
    token.cancel();
    let ctx = EmbedContext::new().with_cancel(&token);
    assert_eq!(model.embed(&["graph"], &ctx), Err(EmbedError::Cancelled));
}

// ---------------------------------------------------------------------------------------------
// Through the DSL
// ---------------------------------------------------------------------------------------------

fn run(graph: &Arc<Graph>, embedders: &Arc<Embedders>, script: &str) -> Dynamic {
    let mut engine = graphersal::script::engine();
    graphersal::script::set_engine_embedders(&mut engine, embedders.clone());
    let mut scope = graphersal::script::graph_scope(graph.clone());
    graphersal::script::set_scope_embedders(&mut scope, embedders.clone());
    engine
        .eval_with_scope::<Dynamic>(&mut scope, script)
        .unwrap_or_else(|err| panic!("{script}: {err}"))
}

#[test]
fn the_embed_step_runs_the_model() {
    let dir = tiny_model("dsl");
    let model = Arc::new(Model2Vec::load("tiny", &dir).unwrap());
    let embedders = Arc::new(
        Embedders::builder()
            .with(model.clone())
            .unwrap()
            .default_name("tiny")
            .unwrap()
            .build(),
    );
    let graph = Arc::new(GraphSource::tinkerpop_modern());

    let count = run(
        &graph,
        &embedders,
        r#"g.v().has_label("software").embed("name").to_list().len()"#,
    );
    assert_eq!(count.as_int().unwrap(), 2);

    // The nearest software to "ripple" by its name embedding, with the explicit embedder.
    let script = r#"
        g.v().has_label("software").property("emb", __.embed("name", #{embedder: "tiny"})).iterate();
        g.v().has_label("software").nearest("emb", embed("ripple", #{embedder: "tiny"}), 1).values("name").to_list()
    "#;
    let nearest = run(&graph, &embedders, script);
    assert_eq!(nearest.into_array().unwrap()[0].to_string(), "ripple");

    // The function embed(text) on the default embedder equals the Rust call.
    let text = run(&graph, &embedders, r#"embed("graph vector").to_string()"#).to_string();
    let vector = model
        .embed(&["graph vector"], &EmbedContext::new())
        .unwrap();
    assert_eq!(text, vector[0].to_string());

    // g.embedders() reports the model identity.
    let listed = run(&graph, &embedders, r#"g.embedders()"#).to_string();
    assert!(listed.contains(&model.info().model.to_string()), "{listed}");
}

// ---------------------------------------------------------------------------------------------
// A real model (not run by default)
// ---------------------------------------------------------------------------------------------

/// Runs against a real `minishlab/potion-base-8M` downloaded beforehand:
///
/// ```text
/// graphersal embed-model download minishlab/potion-base-8M
/// GRAPHERSAL_EMBED_MODELS=~/.cache/graphersal/embed-models \
///   cargo test -p graphersal --features script,model2vec --test all \
///   model2vec_tests::a_real_potion_model -- --ignored
/// ```
#[test]
#[ignore = "needs minishlab/potion-base-8M under $GRAPHERSAL_EMBED_MODELS"]
fn a_real_potion_model() {
    let Some(root) = std::env::var_os("GRAPHERSAL_EMBED_MODELS") else {
        panic!("set GRAPHERSAL_EMBED_MODELS to the models directory");
    };
    let dir = PathBuf::from(root).join("minishlab").join("potion-base-8M");
    let model = Model2Vec::load("potion", &dir).unwrap();
    assert_eq!(model.info().dimension, 256);
    assert!(
        model
            .info()
            .model
            .starts_with("model2vec/minishlab/potion-base-8M@"),
        "{}",
        model.info().model
    );
    let vectors = embed(
        &model,
        &["a graph database", "a graph store", "banana bread"],
    )
    .unwrap();
    let close = vectors[0]
        .similarity(&vectors[1], VectorMetric::Cosine)
        .unwrap();
    let far = vectors[0]
        .similarity(&vectors[2], VectorMetric::Cosine)
        .unwrap();
    assert!(close > far, "{close} <= {far}");
}
