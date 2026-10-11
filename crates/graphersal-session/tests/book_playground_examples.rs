//! Every code block of the book that names its sample graph runs on that sample, in a fresh
//! session, as the playground runs a link (`book/playground-links.js` turns the block into
//! "Run in the playground" links; both sides split a block the same way):
//!
//! - ```` ```rhai,graph=<sample> ````: every statement of the block is its own link and runs on
//!   its own (a block with one statement is one link);
//! - ```` ```rhai,graph=<sample>,script ````: the whole block is one script, one link.
//!
//! A statement whose comment says it fails (`// error`, `// fails`, `// refused`, `// denied`)
//! must fail; every other one must succeed.

use std::path::{Path, PathBuf};

use graphersal_session::{RunOptions, Session};
use serde_json::Value;

const SAMPLES: [&str; 4] = ["modern", "empty", "large", "tree"];

struct Block {
    file: PathBuf,
    line: usize,
    graph: String,
    script: bool,
    body: Vec<String>,
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("book/src") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            markdown_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// The marked blocks of one page: the fence's info string is `rhai,graph=<sample>[,script]`.
fn blocks_in(file: &Path) -> Vec<Block> {
    let text = std::fs::read_to_string(file).expect("page");
    let mut blocks = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let Some(info) = line.trim_start().strip_prefix("```") else {
            continue;
        };
        let attrs: Vec<&str> = info.split(',').map(str::trim).collect();
        let marker = attrs.iter().find(|a| a.starts_with("graph="));
        let mut body = Vec::new();
        for (_, inner) in lines.by_ref() {
            if inner.trim_start().starts_with("```") {
                break;
            }
            body.push(inner.to_string());
        }
        let Some(marker) = marker else { continue };
        let at = format!("{}:{}", file.display(), n + 1);
        assert_eq!(attrs[0], "rhai", "{at}: a playground block must be rhai");
        blocks.push(Block {
            file: file.to_path_buf(),
            line: n + 1,
            graph: marker
                .strip_prefix("graph=")
                .unwrap_or_default()
                .to_string(),
            script: attrs.contains(&"script"),
            body,
        });
    }
    blocks
}

/// How much a line opens (`(`, `[`, `{`) minus what it closes, outside strings and comments.
fn depth_delta(line: &str) -> i32 {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '`' => quote = Some(c),
                '/' if chars.peek() == Some(&'/') => break,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            },
        }
    }
    depth
}

/// The statements of a block, as `book/playground-links.js` splits it: a statement starts at a
/// line without indentation that does not continue the previous one (it does not start with
/// `.`, `)`, `]` or `}`, and every bracket before it is closed); comment lines before a
/// statement belong to it; blank lines separate nothing. Returns (first line index, text).
fn statements(body: &[String]) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, Vec<&str>)> = Vec::new();
    let mut depth = 0;
    for (n, line) in body.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let continues = line.starts_with(char::is_whitespace)
            || line.trim_start().starts_with(['.', ')', ']', '}']);
        let only_comments = out
            .last()
            .is_some_and(|(_, l)| l.iter().all(|l| l.trim_start().starts_with("//")));
        if out.is_empty() || (depth == 0 && !continues && !only_comments) {
            out.push((n, Vec::new()));
        }
        if let Some((_, lines)) = out.last_mut() {
            lines.push(line);
        }
        depth += depth_delta(line);
    }
    out.into_iter()
        .filter(|(_, l)| !l.iter().all(|l| l.trim_start().starts_with("//")))
        .map(|(n, l)| (n, l.join("\n")))
        .collect()
}

/// A statement's comment says it fails: it has the word `fails`, `refused` or `denied`, starts
/// with `error`, or says `error:` (`// cast error: ...`); a JSON key such as `"error":` in a
/// documented result does not count.
fn expects_error(text: &str) -> bool {
    text.lines().any(|line| {
        line.find("//").is_some_and(|at| {
            let comment = line[at + 2..].trim().to_lowercase();
            comment.starts_with("error")
                || comment.contains("error:")
                || comment
                    .split(|c: char| !c.is_alphanumeric())
                    .any(|w| matches!(w, "fails" | "refused" | "denied"))
        })
    })
}

#[test]
fn statements_split_like_the_book_script() {
    let body: Vec<String> = [
        "// leading comment",
        "g.v().count()        // 6",
        "g.v().has(\"name\", \"marko\")",
        "  .out(\"knows\")",
        ".values(\"name\")",
        "",
        "g.inject(\"a // b\", (",
        "1))",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let s = statements(&body);
    assert_eq!(s.len(), 3, "{s:?}");
    assert_eq!(
        s[0],
        (
            0,
            "// leading comment\ng.v().count()        // 6".to_string()
        )
    );
    assert_eq!(s[1].0, 2);
    assert!(s[1].1.ends_with(".values(\"name\")"));
    assert_eq!(s[2].0, 6);
    assert!(expects_error("g.v().foo()   // error: no such step"));
    assert!(expects_error(
        "g.v().values(\"age\").order().by(T.value)   // cast error: expected"
    ));
    assert!(!expects_error("g.v()   // terrorists are not to blame"));
    assert!(!expects_error(
        r#"g.v().count().execute().to_json()   // {"results":[6],"error":null}"#
    ));
}

#[test]
fn every_playground_block_of_the_book_runs_on_its_sample() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../book/src");
    let mut files = Vec::new();
    markdown_files(&src, &mut files);
    files.sort();
    let blocks: Vec<Block> = files.iter().flat_map(|f| blocks_in(f)).collect();
    assert!(
        blocks.len() >= 20,
        "only {} playground blocks found",
        blocks.len()
    );

    let options = RunOptions::default();
    let mut failures = Vec::new();
    let mut runs = 0;
    for b in &blocks {
        let file = b
            .file
            .strip_prefix(&src)
            .unwrap_or(&b.file)
            .display()
            .to_string();
        if !SAMPLES.contains(&b.graph.as_str()) {
            failures.push(format!(
                "{file}:{}: unknown sample graph {:?}",
                b.line, b.graph
            ));
            continue;
        }
        let units = if b.script {
            vec![(0, b.body.join("\n"))]
        } else {
            statements(&b.body)
        };
        for (offset, text) in units {
            runs += 1;
            let at = format!("{file}:{}", b.line + 1 + offset);
            let mut session = Session::new();
            session.load_sample(&b.graph).expect("sample graph");
            let r: Value = serde_json::from_str(&session.execute(&text, &options)).expect("JSON");
            let ok = r["ok"] == true;
            let expected_error = !b.script && expects_error(&text);
            if ok == expected_error {
                let why = if ok {
                    "succeeds, but its comment says it fails".to_string()
                } else {
                    let d = &r["execution"]["error"]["diagnostic"];
                    let text = d.as_str().map_or_else(|| r.to_string(), str::to_string);
                    text.chars().take(400).collect()
                };
                failures.push(format!("{at} on {}: {why}", b.graph));
            }
        }
    }
    assert!(runs >= blocks.len());
    assert!(
        failures.is_empty(),
        "playground examples that fail:\n{}",
        failures.join("\n")
    );
}
