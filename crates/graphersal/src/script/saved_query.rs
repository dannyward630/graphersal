//! Saved queries in the DSL (`book/src/users_guide/saved_queries.md`): defining them
//! (`g.define_query(#{..})`, `g.define_queries(folder, source)`), calling them
//! (`g.query(name, #{..})`) and managing them (`g.queries()`, `g.get_query(name)`,
//! `g.drop_query(name)`, `g.move_query(name, folder)`, `g.describe_query(name, text)`).
//!
//! A saved query is stored in the graph's catalog of definitions
//! ([`crate::catalog`]) as the text of ONE Rhai function. A call:
//!
//! 1. asks the caller's authorizer for `Execute` on the query (before anything runs);
//! 2. binds the named arguments against the parameter schemas (unknown, missing, wrong type or
//!    constraint: an error naming the parameter; defaults substituted; the only conversion is
//!    the schema's write coercion, `int64` -> `float64` and a canonical uuid string);
//! 3. compiles the body, through a per-thread cache keyed by the query's name, dialect,
//!    parameter names and source text, so a changed definition (also one restored by a
//!    rollback) never meets a stale compiled form;
//! 4. runs the function with the caller's engine in a scope whose `g` is the caller's graph
//!    under a READ-ONLY restriction ([`ReadOnly`]): nothing the body runs, and nothing built
//!    from what it returns, can write data, the schema, the catalog or files (a graph the body
//!    builds itself stays writable). Nested calls inherit it; the call depth is bounded by
//!    [`MAX_QUERY_DEPTH`];
//! 5. returns the function's result as is (a traversal can be continued by the caller, a value
//!    is a value); a returned traversal with a writing step is refused.
//!
//! Errors from the body keep their diagnostic and gain a line `in saved query: "name"` (one per
//! nested call), so the origin is named.

use super::*;
use crate::error::{SAVED_QUERY_READ_ONLY, ScriptError};
use crate::graph::definition::{
    Definition, DefinitionKind, QUERY_DIALECT, QueryDefinition, QueryParam,
};
use crate::graph::{AccessRequest, Action, Authorizer, Denied, Resource};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The deepest nesting of saved-query calls (a query calling a query ...); a deeper call fails
/// with [`ScriptError::QueryDepthExceeded`].
pub(crate) const MAX_QUERY_DEPTH: usize = 16;

/// How many compiled bodies the per-thread cache keeps before it starts over.
const CACHE_CAPACITY: usize = 256;

/// The authorizer a saved query runs under: the caller's, minus every write. `Create`,
/// `Update` and `Delete` on data, the schema, the catalog and files are denied with a reason
/// starting with [`SAVED_QUERY_READ_ONLY`]; everything else is the caller's authorizer's
/// decision.
pub(crate) struct ReadOnly {
    inner: Arc<dyn Authorizer>,
}

impl Authorizer for ReadOnly {
    fn authorize(&self, request: &AccessRequest<'_>) -> Result<(), Denied> {
        let writes = matches!(
            request.action,
            Action::Create | Action::Update | Action::Delete
        ) && matches!(
            request.resource,
            Resource::Data { .. }
                | Resource::Schema
                | Resource::Definition { .. }
                | Resource::File { .. }
        );
        if writes {
            return Err(Denied::new(format!(
                "{SAVED_QUERY_READ_ONLY}: it cannot create, change or delete data, the schema, \
                 saved queries or files"
            )));
        }
        self.inner.authorize(request)
    }

    fn resolve_file(&self, path: &std::path::Path) -> Result<std::path::PathBuf, Denied> {
        self.inner.resolve_file(path)
    }
}

/// A compiled body: the AST holding the function and its signature.
struct Compiled {
    ast: rhai::AST,
    signature: Vec<String>,
}

/// What [`compile`] yields: the compiled body or why it is not a valid body.
type CompileResult = Rc<Result<Compiled, String>>;

/// The cache key: everything the compile check depends on.
#[derive(PartialEq, Eq, Hash)]
struct CacheKey {
    name: String,
    dialect: String,
    params: Vec<String>,
    source: String,
}

thread_local! {
    /// Compiled bodies of this thread (Rhai's AST is not `Send`). Keyed by the definition's
    /// text, so every change of a definition (also a rollback) picks its own entry.
    static COMPILED: RefCell<HashMap<CacheKey, CompileResult>> = RefCell::new(HashMap::new());
}

/// The compiled body of the saved query `name`, from the cache or compiled with `engine`.
fn compile(engine: &Engine, name: &str, query: &QueryDefinition) -> CompileResult {
    let key = CacheKey {
        name: name.to_string(),
        dialect: query.dialect().to_string(),
        params: query.params().iter().map(|(p, _)| p.clone()).collect(),
        source: query.source().to_string(),
    };
    if let Some(hit) = COMPILED.with(|cache| cache.borrow().get(&key).cloned()) {
        return hit;
    }
    let compiled = Rc::new(compile_uncached(engine, name, query));
    COMPILED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_CAPACITY {
            cache.clear();
        }
        cache.insert(key, compiled.clone());
    });
    compiled
}

/// Checks the body of the saved query `name` the way a call does, without running it: the
/// dialect is this version's, the source compiles with `engine` and is exactly one function
/// named `name` whose parameters are the declared ones. `Err` is the reason (the `status:
/// "error"` text of `g.queries()`). Front ends use it before storing a definition they built
/// themselves (the playground's editor) and to list a catalog with its status; the compiled
/// form is cached per thread like a call's.
pub fn check_saved_query(
    engine: &Engine,
    name: &str,
    query: &QueryDefinition,
) -> Result<(), String> {
    match compile(engine, name, query).as_ref() {
        Ok(_) => Ok(()),
        Err(reason) => Err(reason.clone()),
    }
}

/// Compiles a body and checks it: the dialect is this version's, the text is exactly one
/// function named `name` (no statements beside it) whose parameters are the declared ones.
fn compile_uncached(
    engine: &Engine,
    name: &str,
    query: &QueryDefinition,
) -> Result<Compiled, String> {
    if query.dialect() != QUERY_DIALECT {
        return Err(format!(
            "its dialect is '{}', this version runs '{QUERY_DIALECT}'",
            query.dialect()
        ));
    }
    let ast = engine
        .compile(query.source())
        .map_err(|err| format!("it does not compile: {err}"))?;
    let signature = single_function(&ast, name)?;
    let declared: Vec<&str> = query.params().iter().map(|(p, _)| p.as_str()).collect();
    check_signature(&signature, &declared)?;
    Ok(Compiled { ast, signature })
}

/// The signature of the one function `ast` defines, which must be named `name`.
fn single_function(ast: &rhai::AST, name: &str) -> Result<Vec<String>, String> {
    if !ast.statements().is_empty() {
        return Err(
            "it has statements outside the function; the body is one function and nothing else"
                .to_string(),
        );
    }
    let functions: Vec<_> = ast.iter_fn_def().collect();
    let function = match functions.as_slice() {
        [] => {
            return Err(format!(
                "it defines no function (expected fn {name}(..) {{ .. }})"
            ));
        }
        [one] => one,
        many => {
            let names: Vec<&str> = many.iter().map(|f| f.name.as_str()).collect();
            return Err(format!(
                "it defines {} functions ({}), a saved query is exactly one",
                many.len(),
                names.join(", ")
            ));
        }
    };
    if function.name.as_str() != name {
        return Err(format!(
            "it defines the function '{}', but the query is named '{name}'",
            function.name
        ));
    }
    if function.this_type.is_some() {
        return Err("a method function (fn Type.name()) cannot be a saved query".to_string());
    }
    Ok(function.params.iter().map(|p| p.to_string()).collect())
}

/// Whether the function's `signature` and the `declared` parameters name the same set.
fn check_signature(signature: &[String], declared: &[&str]) -> Result<(), String> {
    if let Some(missing) = signature.iter().find(|p| !declared.contains(&p.as_str())) {
        return Err(format!(
            "the function's parameter '{missing}' is not declared in params (declare it with a \
             type name such as \"integer\" or with #{{schema: .., default: ..}})"
        ));
    }
    if let Some(extra) = declared.iter().find(|p| !signature.iter().any(|s| s == *p)) {
        return Err(format!(
            "params declares '{extra}', which the function does not take"
        ));
    }
    Ok(())
}

/// The description a function's doc comments give (`///` lines or a `/** .. */` block), with
/// the comment leaders removed; `None` without doc comments.
fn doc_text<S: AsRef<str>>(comments: &[S]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for comment in comments {
        let comment = comment.as_ref();
        if let Some(block) = comment.strip_prefix("/**") {
            let block = block.strip_suffix("*/").unwrap_or(block);
            for line in block.lines() {
                let line = line.trim();
                let line = line.strip_prefix('*').map(str::trim_start).unwrap_or(line);
                lines.push(line.to_string());
            }
        } else {
            for line in comment.lines() {
                let line = line.trim_start();
                let line = line.strip_prefix("///").unwrap_or(line);
                lines.push(
                    line.strip_prefix(' ')
                        .unwrap_or(line)
                        .trim_end()
                        .to_string(),
                );
            }
        }
    }
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The byte offset of a Rhai position (1-based line and character column) in `source`.
fn offset_of(source: &str, position: rhai::Position) -> Option<usize> {
    let line = position.line()?;
    let column = position.position()?;
    let mut start = 0;
    for _ in 1..line {
        start += source[start..].find('\n')? + 1;
    }
    source[start..]
        .char_indices()
        .nth(column.checked_sub(1)?)
        .map(|(index, _)| start + index)
}

/// The text of one function of `source` as a body of its own: `fn name(params) { .. }` with
/// the function's block (from its `{` at `open` to its `}` at `close`) copied verbatim,
/// comments inside it included.
fn function_text(
    source: &str,
    name: &str,
    params: &[String],
    (open, close): (rhai::Position, rhai::Position),
) -> Result<String, String> {
    let located = offset_of(source, open)
        .zip(offset_of(source, close))
        .filter(|(start, end)| {
            start <= end && source[*start..].starts_with('{') && source[*end..].starts_with('}')
        });
    let Some((start, end)) = located else {
        return Err(format!(
            "the body of the function '{name}' could not be located"
        ));
    };
    Ok(format!(
        "fn {name}({}) {}",
        params.join(", "),
        &source[start..=end]
    ))
}

/// A script error with the diagnostic of `err`.
pub(super) fn fail(err: ScriptError) -> Box<EvalAltResult> {
    script_error_from_script(err)
}

/// A script error with the diagnostic of a graph error (a catalog change that failed).
pub(super) fn graph_fail(err: crate::error::GraphError) -> Box<EvalAltResult> {
    script_error(Box::new(crate::TraverserError::from(err)))
}

/// Adds the line `in saved query: "name"` to an error coming out of the query's body (before
/// its `Help:`), so the origin is named; nested calls add one line each, innermost first.
/// Resource-limit and termination errors pass through unchanged, so a host still recognizes
/// them (`resource_limit_from_rhai`, a cancelled run).
fn in_saved_query(err: EvalAltResult, name: &str) -> Box<EvalAltResult> {
    let err = match err {
        EvalAltResult::ErrorInFunctionCall(function, _, inner, _) if function == name => inner,
        other => Box::new(other),
    };
    let (mut text, position) = match *err {
        EvalAltResult::ErrorRuntime(diagnostic, position) if diagnostic.is_string() => {
            (diagnostic.to_string(), position)
        }
        limit @ (EvalAltResult::ErrorTooManyOperations(..)
        | EvalAltResult::ErrorStackOverflow(..)
        | EvalAltResult::ErrorDataTooLarge(..)
        | EvalAltResult::ErrorTooManyVariables(..)
        | EvalAltResult::ErrorTooManyModules(..)
        | EvalAltResult::ErrorTerminated(..)) => return Box::new(limit),
        other => {
            let position = other.position();
            let mut other = map_eval_error(Box::new(other), &ScriptLimits::default());
            other.clear_position();
            (format!("Error: {other}"), position)
        }
    };
    let mut line = format!("  in saved query: \"{name}\"");
    if let (Some(row), Some(column)) = (position.line(), position.position()) {
        line.push_str(&format!(" (line {row}, position {column} of its body)"));
    }
    let at = text.find("\nHelp: ").unwrap_or(text.len());
    add_stack_line(&mut text, at, &line);
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(text),
        rhai::Position::NONE,
    ))
}

/// Inserts the stack line `line` at `at` (the end of the stack, before `Help:`). A line equal
/// to the one before it is counted instead of repeated (`in saved query: "r" (×16)` for a
/// runaway recursion).
fn add_stack_line(text: &mut String, at: usize, line: &str) {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let previous = &text[start..at];
    let count = if previous == line {
        Some(2)
    } else {
        previous
            .strip_prefix(line)
            .and_then(|rest| rest.strip_prefix(" (×"))
            .and_then(|rest| rest.strip_suffix(')'))
            .and_then(|n| n.parse::<u32>().ok())
            .map(|n| n.saturating_add(1))
    };
    match count {
        Some(n) => text.replace_range(start..at, &format!("{line} (×{n})")),
        None => text.insert_str(at, &format!("\n{line}")),
    }
}

/// The first non-data value in a call argument (a traversal, a closure, a token object, ...):
/// its type name, or `None` when the value is data. Bounded by `levels` (a deeper value fails
/// the depth check of the conversion afterwards).
fn non_data(value: &Dynamic, levels: usize) -> Option<String> {
    if value.is_string()
        || value.is_int()
        || value.is_float()
        || value.is_bool()
        || value.is_unit()
        || value.is::<ScriptUuid>()
        || value.is::<ScriptMap>()
    {
        return None;
    }
    let inner = levels.checked_sub(1)?;
    if let Some(items) = value.read_lock::<rhai::Array>() {
        return items.iter().find_map(|item| non_data(item, inner));
    }
    if let Some(map) = value.read_lock::<rhai::Map>() {
        return map.values().find_map(|item| non_data(item, inner));
    }
    Some(type_name_of(value))
}

/// Where `value` holds a traversal whose plan writes (also inside arrays and maps, bounded by
/// `levels`): `location` extended by the index or key path, or `None`.
fn returned_write(value: &Dynamic, location: &str, levels: usize) -> Option<String> {
    if let Some(source) = value.read_lock::<ScriptGraphTraversalSource>() {
        return source.plan_writes().then(|| location.to_string());
    }
    let inner = levels.checked_sub(1)?;
    let base = if location == "the result" {
        "result"
    } else {
        location
    };
    if let Some(items) = value.read_lock::<rhai::Array>() {
        return items
            .iter()
            .enumerate()
            .find_map(|(index, item)| returned_write(item, &format!("{base}[{index}]"), inner));
    }
    if let Some(map) = value.read_lock::<rhai::Map>() {
        return map
            .iter()
            .find_map(|(key, item)| returned_write(item, &format!("{base}.{key}"), inner));
    }
    None
}

/// A string argument that may also be `()` (absent): `None` for `()` and for `""`.
pub(super) fn optional_text(
    operation: &str,
    what: &str,
    value: Dynamic,
) -> Result<Option<String>, Box<EvalAltResult>> {
    if value.is_unit() {
        return Ok(None);
    }
    if value.is_string() {
        let text = value.to_string();
        return Ok((!text.is_empty()).then_some(text));
    }
    Err(fail(ScriptError::ArgumentMismatch {
        step_name: operation.to_string(),
        expected: format!("{what} as a string (or () for none)"),
        actual: type_name_of(&value),
    }))
}

/// The parameter list of a definition from a function `signature` and the declarations in
/// `declared` (parameter name -> a type name or `#{schema, default}`), in signature order.
fn params_of(
    ctx: &NativeCallContext,
    query: &str,
    signature: &[String],
    declared: Option<rhai::Map>,
) -> Result<Vec<(String, QueryParam)>, Box<EvalAltResult>> {
    let declared = declared.unwrap_or_default();
    let names: Vec<&str> = declared.keys().map(|key| key.as_str()).collect();
    check_signature(signature, &names).map_err(|reason| {
        fail(ScriptError::InvalidQueryBody {
            query: query.to_string(),
            reason,
        })
    })?;
    let mut params = Vec::with_capacity(signature.len());
    for parameter in signature {
        let Some(declaration) = declared.get(parameter.as_str()) else {
            continue;
        };
        let value = value_arg(ctx, declaration.clone())?;
        let param = QueryParam::from_value(parameter, &value)
            .map_err(|err| graph_fail(crate::error::GraphError::from(err)))?;
        params.push((parameter.clone(), param));
    }
    Ok(params)
}

/// A map argument (or `()` for none), for `operation`.
fn optional_map(
    operation: &str,
    what: &str,
    value: Dynamic,
) -> Result<Option<rhai::Map>, Box<EvalAltResult>> {
    if value.is_unit() {
        return Ok(None);
    }
    if value.is_map() {
        return Ok(Some(value.cast::<rhai::Map>()));
    }
    Err(fail(ScriptError::ArgumentMismatch {
        step_name: operation.to_string(),
        expected: what.to_string(),
        actual: type_name_of(&value),
    }))
}

/// The entry of `queries()` / `get_query()` for a saved query: name, folder, description,
/// params (in signature order: `#{name, schema, default?, required}`), status (`"ok"` or
/// `"error"` with `error`, the reason its body does not run); with `full` also body, dialect
/// and meta.
fn query_entry(engine: &Engine, name: &str, query: &QueryDefinition, full: bool) -> rhai::Map {
    let text = |value: Option<&str>| value.map_or(Dynamic::UNIT, |v| Dynamic::from(v.to_string()));
    let mut map = rhai::Map::new();
    map.insert("name".into(), Dynamic::from(name.to_string()));
    map.insert("folder".into(), text(query.folder()));
    map.insert("description".into(), text(query.description()));
    let params: rhai::Array = query
        .params()
        .iter()
        .map(|(parameter, param)| {
            let mut entry = rhai::Map::new();
            entry.insert("name".into(), Dynamic::from(parameter.clone()));
            entry.insert(
                "schema".into(),
                element_property_to_dynamic(param.schema().to_value()),
            );
            if let Some(default) = param.default_value() {
                entry.insert(
                    "default".into(),
                    element_property_to_dynamic(default.clone()),
                );
            }
            entry.insert("required".into(), Dynamic::from_bool(param.is_required()));
            Dynamic::from_map(entry)
        })
        .collect();
    map.insert("params".into(), Dynamic::from_array(params));
    match compile(engine, name, query).as_ref() {
        Ok(_) => {
            map.insert("status".into(), Dynamic::from("ok".to_string()));
        }
        Err(reason) => {
            map.insert("status".into(), Dynamic::from("error".to_string()));
            map.insert("error".into(), Dynamic::from(reason.clone()));
        }
    }
    if full {
        map.insert("body".into(), Dynamic::from(query.source().to_string()));
        map.insert("dialect".into(), Dynamic::from(query.dialect().to_string()));
        map.insert("meta".into(), text(query.meta()));
    }
    map
}

/// Whether `folder` is `prefix` or a folder below it (`prefix` without a trailing `/`; the
/// empty prefix matches everything).
fn in_folder(folder: Option<&str>, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    folder.is_some_and(|folder| {
        folder == prefix
            || folder
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

impl ScriptGraphTraversalSource {
    /// The graph of this source, or a script error naming `operation`.
    fn catalog_graph(&self, operation: &str) -> Result<&Arc<dyn AnyGraph>, Box<EvalAltResult>> {
        self.g.as_ref().ok_or_else(|| {
            Box::new(EvalAltResult::ErrorRuntime(
                Dynamic::from(format!("{operation}: no graph associated with this source")),
                rhai::Position::NONE,
            ))
        })
    }

    /// Asks the session's authorizer for `action` on the saved query `name` (`None`: every
    /// saved query).
    fn require_query(
        &self,
        operation: &str,
        action: Action<'_>,
        name: Option<&str>,
    ) -> Result<(), Box<EvalAltResult>> {
        self.require(
            operation,
            AccessRequest::new(
                action,
                Resource::definition(Some(DefinitionKind::QUERY), name),
            ),
        )
    }

    /// Stores `queries` (name, definition) in ONE unit (all or none), asking for `Create` on
    /// each new one and `Update` on each replaced one first.
    fn store_queries(
        &self,
        operation: &str,
        queries: Vec<(String, QueryDefinition)>,
    ) -> Result<(), Box<EvalAltResult>> {
        let graph = self.catalog_graph(operation)?;
        for (name, _) in &queries {
            let exists = graph.definition(DefinitionKind::QUERY, name).is_some();
            let action = if exists {
                Action::Update
            } else {
                Action::Create
            };
            self.require_query(operation, action, Some(name))?;
        }
        graph
            .set_definitions(
                queries
                    .into_iter()
                    .map(|(name, query)| Definition::query(name, query))
                    .collect(),
            )
            .map_err(script_error)
    }

    /// `g.define_query(#{name, body, folder?, description?, params?, meta?})` /
    /// `g.defineQuery(..)`: stores (or replaces) a saved query. `body` is the text of one Rhai
    /// function named `name` whose parameters are declared in `params` (parameter name -> a type
    /// name such as `"integer"`, or `#{schema: <schema node>, default: <value>}`); `description`
    /// defaults to the function's doc comment. Checked before anything is stored: the body
    /// compiles and is exactly that function, the parameters match its signature, and
    /// everything the catalog validates (name, folder, sizes, schemas, defaults). A unit of its
    /// own (a savepoint inside a whole-script unit). Asks for `Create` on the query (`Update`
    /// when it replaces one).
    pub fn define_query(
        ctx: NativeCallContext,
        this: &mut Self,
        spec: Dynamic,
    ) -> Result<(), Box<EvalAltResult>> {
        const KEYS: [&str; 6] = ["name", "body", "folder", "description", "params", "meta"];
        let operation = "define_query";
        let actual = type_name_of(&spec);
        let Some(mut spec) = spec.try_cast::<rhai::Map>() else {
            return Err(fail(ScriptError::ArgumentMismatch {
                step_name: operation.to_string(),
                expected: "a map #{name, body, folder?, description?, params?, meta?}".to_string(),
                actual,
            }));
        };
        if let Some(unknown) = spec.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(fail(ScriptError::ArgumentMismatch {
                step_name: operation.to_string(),
                expected: format!("only the keys {}", KEYS.join(", ")),
                actual: format!("the key '{unknown}'"),
            }));
        }
        let mut take = |key: &str| spec.remove(key).unwrap_or(Dynamic::UNIT);
        let name = optional_text(operation, "name", take("name"))?.ok_or_else(|| {
            fail(ScriptError::ArgumentMismatch {
                step_name: operation.to_string(),
                expected: "the key name (the query's name)".to_string(),
                actual: "no name".to_string(),
            })
        })?;
        let body = optional_text(operation, "body", take("body"))?.ok_or_else(|| {
            fail(ScriptError::ArgumentMismatch {
                step_name: operation.to_string(),
                expected: "the key body (the text of one Rhai function)".to_string(),
                actual: "no body".to_string(),
            })
        })?;
        let folder = optional_text(operation, "folder", take("folder"))?;
        let description = optional_text(operation, "description", take("description"))?;
        let meta = optional_text(operation, "meta", take("meta"))?;
        let declared = optional_map(
            operation,
            "params as a map (parameter name -> type name or #{schema, default})",
            take("params"),
        )?;

        // The name first (a keyword or a too long name would only confuse the compile check).
        Definition::query(name.as_str(), QueryDefinition::new("fn"))
            .validate()
            .map_err(|err| graph_fail(crate::error::GraphError::from(err)))?;
        let ast = ctx.engine().compile(&body).map_err(|err| {
            fail(ScriptError::InvalidQueryBody {
                query: name.clone(),
                reason: format!("it does not compile: {err}"),
            })
        })?;
        let invalid = |reason: String| {
            fail(ScriptError::InvalidQueryBody {
                query: name.clone(),
                reason,
            })
        };
        let signature = single_function(&ast, &name).map_err(invalid)?;
        let doc = ast
            .iter_fn_def()
            .next()
            .and_then(|function| doc_text(&function.comments[..]));
        let params = params_of(&ctx, &name, &signature, declared)?;

        let mut query = QueryDefinition::new(body);
        if let Some(folder) = folder {
            query = query.with_folder(folder);
        }
        if let Some(description) = description.or(doc) {
            query = query.with_description(description);
        }
        if let Some(meta) = meta {
            query = query.with_meta(meta);
        }
        for (parameter, param) in params {
            query = query.with_param(parameter, param);
        }
        this.store_queries(operation, vec![(name, query)])
    }

    /// `g.define_queries(folder, source)` / `g.defineQueries(..)`: [`Self::define_queries_with`]
    /// for functions without parameters.
    pub fn define_queries(
        ctx: NativeCallContext,
        this: &mut Self,
        folder: Dynamic,
        source: String,
    ) -> Result<rhai::Array, Box<EvalAltResult>> {
        Self::define_queries_with(ctx, this, folder, source, Dynamic::UNIT)
    }

    /// `g.define_queries(folder, source, params)` / `g.defineQueries(..)`: stores every function
    /// of `source` as a saved query of its own, named like the function, in `folder` (`""` or
    /// `()`: the root), described by its doc comment (`///` lines or a `/** */` block), with
    /// the parameters declared in `params` (function name -> its parameter map, as in
    /// [`Self::define_query`]). All queries are stored in ONE unit (all or none); returns their
    /// names. Asks for `Create` (`Update`) on each.
    pub fn define_queries_with(
        ctx: NativeCallContext,
        this: &mut Self,
        folder: Dynamic,
        source: String,
        params: Dynamic,
    ) -> Result<rhai::Array, Box<EvalAltResult>> {
        let operation = "define_queries";
        let folder = optional_text(operation, "the folder", folder)?;
        let mut declared = optional_map(
            operation,
            "params as a map (function name -> its parameter map)",
            params,
        )?
        .unwrap_or_default();
        let invalid = |query: &str, reason: String| {
            fail(ScriptError::InvalidQueryBody {
                query: query.to_string(),
                reason,
            })
        };
        let ast = ctx
            .engine()
            .compile(&source)
            .map_err(|err| invalid("*", format!("the source does not compile: {err}")))?;
        if !ast.statements().is_empty() {
            return Err(invalid(
                "*",
                "the source has statements outside its functions; it holds functions only"
                    .to_string(),
            ));
        }
        let mut functions: Vec<_> = ast.iter_fn_def().collect();
        functions.sort_by(|a, b| a.name.cmp(&b.name));
        if functions.is_empty() {
            return Err(invalid("*", "the source defines no function".to_string()));
        }
        let mut queries = Vec::with_capacity(functions.len());
        for (index, function) in functions.iter().enumerate() {
            let name = function.name.to_string();
            if index > 0 && functions[index - 1].name == function.name {
                return Err(invalid(
                    &name,
                    "the source defines two functions of this name; a saved query's name is \
                     unique"
                        .to_string(),
                ));
            }
            if function.this_type.is_some() {
                return Err(invalid(
                    &name,
                    "a method function (fn Type.name()) cannot be a saved query".to_string(),
                ));
            }
            let signature: Vec<String> = function.params.iter().map(|p| p.to_string()).collect();
            let text = function_text(
                &source,
                &name,
                &signature,
                (function.body.start_position(), function.body.end_position()),
            )
            .map_err(|reason| invalid(&name, reason))?;
            let own = match declared.remove(name.as_str()) {
                None => None,
                Some(value) => optional_map(operation, "the parameter map of a function", value)?,
            };
            let params = params_of(&ctx, &name, &signature, own)?;
            let mut query = QueryDefinition::new(text);
            if let Some(folder) = &folder {
                query = query.with_folder(folder.clone());
            }
            if let Some(description) = doc_text(&function.comments[..]) {
                query = query.with_description(description);
            }
            for (parameter, param) in params {
                query = query.with_param(parameter, param);
            }
            // The extracted text must stand on its own as the body of the query.
            if let Err(reason) = compile_uncached(ctx.engine(), &name, &query) {
                return Err(invalid(&name, reason));
            }
            queries.push((name, query));
        }
        if let Some(unknown) = declared.keys().next() {
            return Err(invalid(
                unknown,
                "params declares parameters for this name, but the source defines no such \
                 function"
                    .to_string(),
            ));
        }
        let names: rhai::Array = queries
            .iter()
            .map(|(name, _)| Dynamic::from(name.clone()))
            .collect();
        this.store_queries(operation, queries)?;
        Ok(names)
    }

    /// `g.query(name)`: [`Self::query_with`] without arguments (every parameter takes its
    /// default).
    pub fn query(
        ctx: NativeCallContext,
        this: &mut Self,
        name: &str,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        this.call_query(&ctx, name, rhai::Map::new())
    }

    /// `g.query(name, #{param: value, ..})`: calls the saved query `name` with named
    /// arguments and returns its result as is: a traversal (continue it like any other, it
    /// stays read-only) or a value. Only at the start of a traversal, on `g` (not after steps,
    /// not on `__`). See the module docs for the call rules.
    pub fn query_with(
        ctx: NativeCallContext,
        this: &mut Self,
        name: &str,
        args: Dynamic,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        let args = optional_map(
            "query",
            "the named arguments as a map, for example #{limit: 5}",
            args,
        )?
        .unwrap_or_default();
        this.call_query(&ctx, name, args)
    }

    fn call_query(
        &self,
        ctx: &NativeCallContext,
        name: &str,
        mut args: rhai::Map,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        let not_at_start = |position: &str| {
            fail(ScriptError::QueryNotAtStart {
                name: name.to_string(),
                position: position.to_string(),
            })
        };
        if self.anonymous {
            return Err(not_at_start("__ (an anonymous traversal)"));
        }
        if !self.steps.is_empty() {
            return Err(not_at_start("other steps"));
        }
        if self.access.query_depth >= MAX_QUERY_DEPTH {
            return Err(fail(ScriptError::QueryDepthExceeded {
                name: name.to_string(),
                limit: MAX_QUERY_DEPTH,
            }));
        }
        self.require_query("query", Action::Execute, Some(name))?;
        let graph = self.catalog_graph("query")?;
        let definition = graph
            .definition(DefinitionKind::QUERY, name)
            .ok_or_else(|| {
                graph_fail(crate::error::GraphError::from(
                    crate::error::DefinitionError::NotFound {
                        kind: DefinitionKind::QUERY.to_string(),
                        name: name.to_string(),
                    },
                ))
            })?;
        let invalid_body = |reason: String| {
            fail(ScriptError::InvalidQueryBody {
                query: name.to_string(),
                reason,
            })
        };
        let Some(query) = definition.as_query() else {
            return Err(invalid_body(
                "it is kept opaque (this version could not decode it)".to_string(),
            ));
        };
        let compiled = compile(ctx.engine(), name, query);
        let compiled = match compiled.as_ref() {
            Ok(compiled) => compiled,
            Err(reason) => return Err(invalid_body(reason.clone())),
        };

        // The arguments, in signature order.
        if let Some(unknown) = args
            .keys()
            .find(|key| !compiled.signature.iter().any(|p| p == key.as_str()))
        {
            return Err(fail(ScriptError::UnknownQueryArgument {
                query: name.to_string(),
                argument: unknown.to_string(),
                parameters: compiled.signature.clone(),
            }));
        }
        let depth = access::value_depth_of(ctx);
        let mut values = Vec::with_capacity(compiled.signature.len());
        for parameter in &compiled.signature {
            let Some(param) = query.param(parameter) else {
                return Err(invalid_body(format!(
                    "the parameter '{parameter}' is not declared"
                )));
            };
            let invalid = |reason: String| {
                fail(ScriptError::InvalidQueryArgument {
                    query: name.to_string(),
                    parameter: parameter.clone(),
                    reason,
                })
            };
            let value = match args.remove(parameter.as_str()) {
                Some(value) => {
                    if let Some(type_name) = non_data(&value, depth) {
                        return Err(invalid(format!(
                            "a {type_name} is not a data value; arguments are values only"
                        )));
                    }
                    let value = value_arg(ctx, value)?;
                    param
                        .check_value(name, parameter, value)
                        .map_err(|err| invalid(err.to_string()))?
                }
                None => match param.default_value() {
                    Some(default) => default.clone(),
                    None => {
                        return Err(fail(ScriptError::MissingQueryArgument {
                            query: name.to_string(),
                            parameter: parameter.clone(),
                            expected: param.schema().non_null_type_name(),
                        }));
                    }
                },
            };
            values.push(element_property_to_dynamic(value));
        }

        // The body's access: the caller's, read-only, one level deeper.
        let authorizer: Arc<dyn Authorizer> = if self.access.read_only {
            self.access.authorizer.clone()
        } else {
            Arc::new(ReadOnly {
                inner: self.access.authorizer.clone(),
            })
        };
        let access = Arc::new(access::ScriptAccess {
            authorizer,
            presets: self.access.presets.clone(),
            max_value_depth: self.access.max_value_depth,
            read_only: true,
            query_depth: self.access.query_depth + 1,
        });
        let g = ScriptGraphTraversalSource {
            g: Some(graph.clone()),
            steps: Vec::new(),
            options: self.options.clone(),
            access: access.clone(),
            private: false,
            anonymous: false,
            now: None,
        };
        let mut scope = eval::scope_with_access(g, access);
        // The body sees the caller's `now()` (one value per evaluation, `access.rs`).
        let tag = access::nested_tag(ctx, access::ScriptAccess::of_context(ctx));
        let result = ctx
            .engine()
            .call_fn_with_options::<Dynamic>(
                rhai::CallFnOptions::new()
                    .eval_ast(false)
                    .rewind_scope(true)
                    .with_tag(tag),
                &mut scope,
                &compiled.ast,
                name,
                values,
            )
            .map_err(|err| in_saved_query(*err, name))?;
        if let Some(location) = returned_write(&result, "the result", depth) {
            return Err(fail(ScriptError::QueryReturnedWrite {
                query: name.to_string(),
                location,
            }));
        }
        Ok(result)
    }

    /// `g.queries()`: every saved query as a list of maps, by name (name,
    /// folder, description, params, status and error). Asks for `Read` on the saved queries.
    pub fn queries(
        ctx: NativeCallContext,
        this: &mut Self,
    ) -> Result<rhai::Array, Box<EvalAltResult>> {
        Self::queries_in(ctx, this, String::new())
    }

    /// `g.queries(folder)`: the saved queries in `folder` and the folders below it
    /// (`"reports"` lists `reports` and `reports/sales`; a trailing `/` is ignored).
    pub fn queries_in(
        ctx: NativeCallContext,
        this: &mut Self,
        folder: String,
    ) -> Result<rhai::Array, Box<EvalAltResult>> {
        this.require_query("queries", Action::Read, None)?;
        let prefix = folder.trim().trim_end_matches('/');
        let definitions = this
            .catalog_graph("queries")?
            .definitions(Some(DefinitionKind::QUERY));
        Ok(definitions
            .iter()
            .filter_map(|definition| {
                let query = definition.as_query()?;
                in_folder(query.folder(), prefix).then(|| {
                    Dynamic::from_map(query_entry(ctx.engine(), definition.name(), query, false))
                })
            })
            .collect())
    }

    /// `g.get_query(name)` / `g.getQuery(name)`: the saved query as a map (the entry of
    /// [`Self::queries`] plus `body`, `dialect` and `meta`), `()` when there is none. Asks for
    /// `Read` on it.
    pub fn get_query(
        ctx: NativeCallContext,
        this: &mut Self,
        name: &str,
    ) -> Result<Dynamic, Box<EvalAltResult>> {
        this.require_query("get_query", Action::Read, Some(name))?;
        let definition = this
            .catalog_graph("get_query")?
            .definition(DefinitionKind::QUERY, name);
        Ok(definition
            .as_ref()
            .and_then(Definition::as_query)
            .map_or(Dynamic::UNIT, |query| {
                Dynamic::from_map(query_entry(ctx.engine(), name, query, true))
            }))
    }

    /// `g.drop_query(name)` / `g.dropQuery(name)`: removes the saved query; `false` when there
    /// is none. A unit of its own; asks for `Delete` on it. Callers of a dropped query fail
    /// when they call it (dependencies are not tracked).
    pub fn drop_query(&mut self, name: &str) -> Result<bool, Box<EvalAltResult>> {
        self.require_query("drop_query", Action::Delete, Some(name))?;
        self.catalog_graph("drop_query")?
            .remove_definition(DefinitionKind::QUERY, name)
            .map_err(script_error)
    }

    /// `g.move_query(name, folder)` / `g.moveQuery(..)`: moves the saved query into `folder`
    /// (`""` or `()`: the root). Callers name a query without its folder, so moving breaks
    /// none. Asks for `Update` on it.
    pub fn move_query(&mut self, name: &str, folder: Dynamic) -> Result<(), Box<EvalAltResult>> {
        let folder = optional_text("move_query", "the folder", folder)?;
        self.require_query("move_query", Action::Update, Some(name))?;
        self.catalog_graph("move_query")?
            .move_query(name, folder.as_deref())
            .map_err(script_error)
    }

    /// `g.describe_query(name, text)` / `g.describeQuery(..)`: sets the description of the
    /// saved query (`""` or `()` removes it). Asks for `Update` on it.
    pub fn describe_query(
        &mut self,
        name: &str,
        description: Dynamic,
    ) -> Result<(), Box<EvalAltResult>> {
        let description = optional_text("describe_query", "the description", description)?;
        self.require_query("describe_query", Action::Update, Some(name))?;
        self.catalog_graph("describe_query")?
            .describe_query(name, description.as_deref())
            .map_err(script_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comments_become_descriptions() {
        assert_eq!(
            doc_text(&["/// First line.\n/// Second.".to_string()]).as_deref(),
            Some("First line.\nSecond.")
        );
        assert_eq!(
            doc_text(&["/** Block\n * doc */".to_string()]).as_deref(),
            Some("Block\ndoc")
        );
        assert_eq!(doc_text::<String>(&[]), None);
    }

    #[test]
    fn folders_match_by_segment() {
        assert!(in_folder(Some("reports"), "reports"));
        assert!(in_folder(Some("reports/sales"), "reports"));
        assert!(!in_folder(Some("reportsx"), "reports"));
        assert!(!in_folder(None, "reports"));
        assert!(in_folder(None, ""));
    }

    #[test]
    fn function_text_is_cut_from_the_source() {
        let engine = engine();
        let source = "\n/// d\nfn a(x, y) {\n  x + y // }\n}\nfn b() { `}` }\n";
        let ast = engine.compile(source).unwrap();
        let mut texts: Vec<String> = ast
            .iter_fn_def()
            .map(|f| {
                let params: Vec<String> = f.params.iter().map(|p| p.to_string()).collect();
                function_text(
                    source,
                    &f.name,
                    &params,
                    (f.body.start_position(), f.body.end_position()),
                )
                .unwrap()
            })
            .collect();
        texts.sort();
        assert_eq!(texts, ["fn a(x, y) {\n  x + y // }\n}", "fn b() { `}` }"]);
    }
}

#[cfg(test)]
mod stack_line_tests {
    use super::add_stack_line;

    fn add(text: &str, line: &str) -> String {
        let mut text = text.to_string();
        let at = text.find("\nHelp: ").unwrap_or(text.len());
        add_stack_line(&mut text, at, line);
        text
    }

    #[test]
    fn equal_lines_are_counted_different_ones_kept() {
        let a = "  in saved query: \"r\"";
        let b = "  in saved query: \"outer\"";
        let once = add("Error: x\nHelp: fix", a);
        assert_eq!(once, "Error: x\n  in saved query: \"r\"\nHelp: fix");
        let twice = add(&once, a);
        assert_eq!(twice, "Error: x\n  in saved query: \"r\" (×2)\nHelp: fix");
        let thrice = add(&twice, a);
        assert_eq!(thrice, "Error: x\n  in saved query: \"r\" (×3)\nHelp: fix");
        let outer = add(&thrice, b);
        assert_eq!(
            outer,
            "Error: x\n  in saved query: \"r\" (×3)\n  in saved query: \"outer\"\nHelp: fix"
        );
        assert_eq!(add("Error: x", a), "Error: x\n  in saved query: \"r\"");
    }
}
