//! `graphersal embed-model download|list` and the `--embedder NAME=PATH|REPO` resolution
//! (feature `model2vec`; book pages "Embeddings" and "Command line").
//!
//! The only network access of the binary besides the AI Chat: `download` fetches the three
//! files of a Model2Vec model (`config.json`, `tokenizer.json`, `model.safetensors`) from
//! `<endpoint>/<repo>/resolve/<revision>/<file>` (endpoint `https://huggingface.co`, or
//! `GRAPHERSAL_HF_ENDPOINT` for a mirror or a test; `HF_TOKEN` as a bearer token, sent to that
//! endpoint's host only, never logged). Each file streams into a `.part` file of a hidden
//! staging directory while its SHA-256 is computed; the hash Hugging Face states for an LFS
//! file (`X-Linked-Etag`) and the size are checked, the file renamed; the manifest
//! [`Model2Vec::MANIFEST_FILE`] is written, the model loaded once as a check, and only then the
//! staging directory renamed to `<models-dir>/<repo>`. A failure (or a kill) leaves no model
//! behind: at most a hidden staging directory, which the next download removes.

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use graphersal::vector::{Embedder, Model2Vec};
use serde_json::{Value, json};

use crate::cli::{EXIT_OK, EXIT_QUERY_ERROR, EXIT_USAGE_ERROR};

/// The files a Model2Vec model in the Model2Vec layout consists of, in download order.
pub const MODEL_FILES: [&str; 3] = ["config.json", "tokenizer.json", "model.safetensors"];

/// The default Hugging Face endpoint.
const DEFAULT_ENDPOINT: &str = "https://huggingface.co";

/// The largest file a download accepts when the server states no size.
const MAX_FILE_BYTES: u64 = 8 << 30;

/// Usage of `graphersal embed-model`.
pub const EMBED_MODEL_HELP: &str = r#"graphersal embed-model - download and list Model2Vec embedding models

USAGE:
    graphersal embed-model download <REPO> [--revision <REV>] [--dir <MODELS-DIR>]
    graphersal embed-model list [--dir <MODELS-DIR>]

COMMANDS:
    download   Fetch config.json, tokenizer.json and model.safetensors of the Hugging Face
               repository REPO (for example minishlab/potion-base-8M) into
               <MODELS-DIR>/<REPO>/, with a manifest graphersal-embed-model.json (repository,
               revision, commit, files with sizes and SHA-256). The SHA-256 Hugging Face states
               for a large file is checked; nothing is left behind when the download fails.
               --revision: a branch, tag or commit (default main).
    list       The downloaded models: repository, size and model identity.

    The models directory is --dir, else $GRAPHERSAL_EMBED_MODELS, else the user cache:
    $XDG_CACHE_HOME/graphersal/embed-models or ~/.cache/graphersal/embed-models
    (%LOCALAPPDATA%\graphersal\embed-models on Windows).

    Use a model with --embedder NAME=REPO (or NAME=<directory>) on any graphersal command:
        graphersal --embedder potion=minishlab/potion-base-8M --default-embedder potion \
            -e 'g.v().values("name").embed().to_list()'

NETWORK:
    download is the only command that connects anywhere: to https://huggingface.co (or
    $GRAPHERSAL_HF_ENDPOINT); $HF_TOKEN, when set, is sent as a bearer token for gated or
    private repositories. Loading and running a model never uses the network. A model's
    licence is the model's own (see its repository).

EXIT CODES:
    0  done
    1  the download or a model failed
    2  invalid invocation
"#;

/// Runs `graphersal embed-model <args>`; returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let Some(command) = args.first() else {
        out!("{EMBED_MODEL_HELP}");
        return EXIT_USAGE_ERROR;
    };
    let rest = &args[1..];
    if matches!(command.as_str(), "help" | "--help" | "-h")
        || rest.iter().any(|a| a == "--help" || a == "-h")
    {
        out!("{EMBED_MODEL_HELP}");
        return EXIT_OK;
    }
    let parsed = match parse_flags(rest, &["revision", "dir"]) {
        Ok(parsed) => parsed,
        Err(message) => return usage(&message),
    };
    match command.as_str() {
        "download" => {
            let [repo] = parsed.positional.as_slice() else {
                return usage("embed-model download takes one repository (owner/name)");
            };
            let dir = match models_dir(parsed.flag("dir")) {
                Ok(dir) => dir,
                Err(message) => return usage(&message),
            };
            let revision = parsed.flag("revision").unwrap_or("main");
            match download(repo, revision, &dir) {
                Ok(()) => EXIT_OK,
                Err(DownloadError::Usage(message)) => usage(&message),
                Err(DownloadError::Failed(message)) => {
                    errln!("Error: {message}");
                    EXIT_QUERY_ERROR
                }
            }
        }
        "list" => {
            if parsed.flag("revision").is_some() || !parsed.positional.is_empty() {
                return usage("embed-model list takes only --dir");
            }
            match models_dir(parsed.flag("dir")) {
                Ok(dir) => list(&dir),
                Err(message) => usage(&message),
            }
        }
        other => usage(&format!("Unknown embed-model command: {other}")),
    }
}

fn usage(message: &str) -> i32 {
    errln!("{message}\n\nRun `graphersal embed-model help` for usage.");
    EXIT_USAGE_ERROR
}

/// Positional arguments and `--name value` / `--name=value` flags.
struct Parsed {
    positional: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Parsed {
    fn flag(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

fn parse_flags(args: &[String], known: &[&str]) -> Result<Parsed, String> {
    let mut parsed = Parsed {
        positional: Vec::new(),
        flags: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if let Some(flag) = arg.strip_prefix("--") {
            let (name, inline) = match flag.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (flag, None),
            };
            if !known.contains(&name) {
                return Err(format!("Unknown option: --{name}"));
            }
            let value = match inline {
                Some(value) => value,
                None => args
                    .next()
                    .cloned()
                    .ok_or_else(|| format!("Option --{name} requires a value"))?,
            };
            parsed.flags.push((name.to_string(), value));
        } else {
            parsed.positional.push(arg.clone());
        }
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------------------------
// The models directory and --embedder
// ---------------------------------------------------------------------------------------------

/// A non-empty environment variable.
fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// The models directory: `explicit`, else `$GRAPHERSAL_EMBED_MODELS`, else the user cache.
pub fn models_dir(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(dir) = explicit {
        return Ok(PathBuf::from(dir));
    }
    if let Some(dir) = env("GRAPHERSAL_EMBED_MODELS") {
        return Ok(PathBuf::from(dir));
    }
    if cfg!(windows)
        && let Some(local) = env("LOCALAPPDATA")
    {
        return Ok(PathBuf::from(local).join("graphersal").join("embed-models"));
    }
    if let Some(cache) = env("XDG_CACHE_HOME").filter(|c| Path::new(c).is_absolute()) {
        return Ok(PathBuf::from(cache).join("graphersal").join("embed-models"));
    }
    if let Some(home) = env("HOME").or_else(|| env("USERPROFILE")) {
        return Ok(PathBuf::from(home)
            .join(".cache")
            .join("graphersal")
            .join("embed-models"));
    }
    Err(
        "No models directory: set GRAPHERSAL_EMBED_MODELS (or pass --dir); neither HOME nor \
         XDG_CACHE_HOME is set"
            .to_string(),
    )
}

/// Whether `repo` is a Hugging Face repository name: `owner/name` (or a bare `name`), each part
/// of ASCII letters, digits, `.`, `_`, `-`, not starting with `.`.
pub fn is_repo_name(repo: &str) -> bool {
    let part_ok = |part: &str| {
        !part.is_empty()
            && part.len() <= 96
            && !part.starts_with('.')
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    let mut parts = repo.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(name), None, None) => part_ok(name),
        (Some(owner), Some(name), None) => part_ok(owner) && part_ok(name),
        _ => false,
    }
}

/// Loads the model of `--embedder NAME=SPEC`: SPEC is a model directory, or a repository
/// downloaded into the models directory.
pub fn load_embedder(name: &str, spec: &str) -> Result<Model2Vec, String> {
    let path = Path::new(spec);
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else if is_repo_name(spec) {
        let models = models_dir(None)?;
        let dir = models.join(spec);
        if !dir.is_dir() {
            return Err(format!(
                "--embedder {name}={spec}: '{spec}' is neither a model directory nor a \
                 downloaded model (looked in {})\nHelp: run `graphersal embed-model download \
                 {spec}`, or pass the directory of a Model2Vec model",
                dir.display()
            ));
        }
        dir
    } else {
        return Err(format!(
            "--embedder {name}={spec}: no such directory, and '{spec}' is not a Hugging Face \
             repository name (owner/name)\nHelp: pass the directory of a Model2Vec model, or \
             a repository downloaded with `graphersal embed-model download <owner/name>`"
        ));
    };
    Model2Vec::load(name, &dir).map_err(|err| {
        format!(
            "--embedder {name}={spec}: {err}\nHelp: {}",
            err.help()
                .lines()
                .next()
                .unwrap_or("check the model directory")
        )
    })
}

// ---------------------------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------------------------

/// A downloaded model found under the models directory.
struct Listed {
    repo: String,
    bytes: u64,
    identity: Result<String, String>,
}

/// The model directories under `dir` (at most two levels: `name` and `owner/name`) that hold a
/// manifest; hidden directories (staging) are skipped.
fn find_models(dir: &Path) -> Vec<Listed> {
    let mut found = Vec::new();
    let visible_dirs = |dir: &Path| -> Vec<(String, PathBuf)> {
        let mut dirs: Vec<(String, PathBuf)> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                (!name.starts_with('.')).then(|| (name, entry.path()))
            })
            .collect();
        dirs.sort();
        dirs
    };
    let mut visit = |repo: String, path: PathBuf| {
        let manifest = path.join(Model2Vec::MANIFEST_FILE);
        let Ok(text) = fs::read_to_string(&manifest) else {
            return false;
        };
        let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let repo = value
            .get("repo")
            .and_then(Value::as_str)
            .map_or(repo, str::to_string);
        let bytes = MODEL_FILES
            .iter()
            .filter_map(|file| fs::metadata(path.join(file)).ok())
            .map(|meta| meta.len())
            .sum();
        let identity = Model2Vec::model_identity(&path).map_err(|err| err.to_string());
        found.push(Listed {
            repo,
            bytes,
            identity,
        });
        true
    };
    for (first, path) in visible_dirs(dir) {
        if visit(first.clone(), path.clone()) {
            continue;
        }
        for (second, inner) in visible_dirs(&path) {
            visit(format!("{first}/{second}"), inner);
        }
    }
    found
}

fn list(dir: &Path) -> i32 {
    let models = find_models(dir);
    if models.is_empty() {
        errln!(
            "No downloaded models in {} (graphersal embed-model download <owner/name>)",
            dir.display()
        );
        return EXIT_OK;
    }
    let width = models
        .iter()
        .map(|m| m.repo.len())
        .max()
        .unwrap_or(4)
        .max(4);
    outln!("{:width$}  {:>9}  IDENTITY", "REPO", "SIZE");
    for model in &models {
        let identity = match &model.identity {
            Ok(identity) => identity.clone(),
            Err(err) => format!("(unreadable: {err})"),
        };
        outln!(
            "{:width$}  {:>9}  {identity}",
            model.repo,
            human_bytes(model.bytes)
        );
    }
    EXIT_OK
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

// ---------------------------------------------------------------------------------------------
// download
// ---------------------------------------------------------------------------------------------

enum DownloadError {
    /// Invalid arguments (exit 2).
    Usage(String),
    /// The download failed (exit 1).
    Failed(String),
}

fn failed(message: impl Into<String>) -> DownloadError {
    DownloadError::Failed(message.into())
}

/// One downloaded file.
struct Fetched {
    name: &'static str,
    size: u64,
    sha256: String,
    /// The commit the server resolved the revision to (`X-Repo-Commit`).
    commit: Option<String>,
}

/// A staging directory removed on drop unless it was moved into place.
struct Staging {
    path: PathBuf,
    done: bool,
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.done {
            let _ = fs::remove_dir_all(&self.path);
            // The owner directory a failed first download created (removed only when empty).
            if let Some(parent) = self.path.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }
}

fn download(repo: &str, revision: &str, models: &Path) -> Result<(), DownloadError> {
    if !is_repo_name(repo) {
        return Err(DownloadError::Usage(format!(
            "'{repo}' is not a Hugging Face repository name: owner/name of letters, digits, \
             '.', '_' and '-' (for example minishlab/potion-base-8M)"
        )));
    }
    if revision.is_empty()
        || revision.starts_with('/')
        || revision.contains("..")
        || !revision
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return Err(DownloadError::Usage(format!(
            "'{revision}' is not a revision: a branch, tag or commit of letters, digits, '.', \
             '_', '-' and '/'"
        )));
    }
    let endpoint = env("GRAPHERSAL_HF_ENDPOINT")
        .unwrap_or_else(|| DEFAULT_ENDPOINT.to_string())
        .trim_end_matches('/')
        .to_string();
    let token = env("HF_TOKEN");
    let target = models.join(repo);
    let parent = target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| models.to_path_buf());
    fs::create_dir_all(&parent)
        .map_err(|err| failed(format!("cannot create {}: {err}", parent.display())))?;
    let leaf = repo.rsplit('/').next().unwrap_or(repo);
    remove_stale_staging(&parent, leaf);
    let mut staging = Staging {
        path: parent.join(format!(".{leaf}.download-{}", std::process::id())),
        done: false,
    };
    let _ = fs::remove_dir_all(&staging.path);
    fs::create_dir_all(&staging.path)
        .map_err(|err| failed(format!("cannot create {}: {err}", staging.path.display())))?;

    errln!("Downloading {repo} (revision {revision}) from {endpoint}");
    let client = Client::new(&endpoint, token);
    let encoded_revision = revision.replace('/', "%2F");
    let mut fetched = Vec::new();
    for name in MODEL_FILES {
        let url = format!("{endpoint}/{repo}/resolve/{encoded_revision}/{name}");
        fetched.push(client.fetch(&url, repo, revision, name, &staging.path)?);
    }

    let commit = fetched.iter().find_map(|f| f.commit.clone());
    let manifest = json!({
        "format": 1,
        "repo": repo,
        "revision": revision,
        "commit": commit,
        "files": fetched.iter().map(|f| json!({
            "name": f.name,
            "size": f.size,
            "sha256": f.sha256,
        })).collect::<Vec<_>>(),
        "downloaded_at": now_text(),
    });
    let manifest_path = staging.path.join(Model2Vec::MANIFEST_FILE);
    write_synced(
        &manifest_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).unwrap_or_default()
        )
        .as_bytes(),
    )
    .map_err(|err| failed(format!("cannot write {}: {err}", manifest_path.display())))?;

    // Load it once: a repository that is not a Model2Vec model fails here, not later.
    let model = Model2Vec::load("check", &staging.path).map_err(|err| {
        failed(format!(
            "{repo} was downloaded but does not load as a Model2Vec model: {err}"
        ))
    })?;

    install(&staging, &target)?;
    staging.done = true;
    outln!("{repo} -> {}", target.display());
    outln!(
        "identity {}, dimension {}",
        model.info().model,
        model.info().dimension
    );
    Ok(())
}

/// Moves the finished staging directory to `target`, replacing an older download.
fn install(staging: &Staging, target: &Path) -> Result<(), DownloadError> {
    let old = target.with_file_name(format!(
        ".{}.old-{}",
        target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        std::process::id()
    ));
    let replaced = target.exists();
    if replaced {
        fs::rename(target, &old).map_err(|err| {
            failed(format!(
                "cannot replace the earlier download {}: {err}",
                target.display()
            ))
        })?;
    }
    if let Err(err) = fs::rename(&staging.path, target) {
        if replaced {
            let _ = fs::rename(&old, target);
        }
        return Err(failed(format!(
            "cannot move the model to {}: {err}",
            target.display()
        )));
    }
    if replaced {
        let _ = fs::remove_dir_all(&old);
    }
    if let Some(parent) = target.parent() {
        sync_dir(parent);
    }
    Ok(())
}

/// Removes the leftovers of interrupted downloads of `leaf` in `parent`.
fn remove_stale_staging(parent: &Path, leaf: &str) {
    let prefixes = [format!(".{leaf}.download-"), format!(".{leaf}.old-")];
    for entry in fs::read_dir(parent).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if prefixes.iter().any(|p| name.starts_with(p.as_str())) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(handle) = fs::File::open(dir) {
        let _ = handle.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

/// Now, as the canonical UTC date-time text.
fn now_text() -> String {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_micros()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    graphersal::temporal::DateTime::from_micros(micros - micros % 1_000_000, 0)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// The HTTP client of a download.
struct Client {
    agent: ureq::Agent,
    /// The authority (`host[:port]`) of the endpoint: the only one `HF_TOKEN` is sent to.
    endpoint_authority: String,
    token: Option<String>,
}

/// The `scheme://authority` part and the authority of an absolute URL.
fn origin_of(url: &str) -> Option<(&str, &str)> {
    let scheme_end = url.find("://")?;
    let rest = &url[scheme_end + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let origin_len = scheme_end + 3 + authority_end;
    Some((&url[..origin_len], &rest[..authority_end]))
}

/// `location` (a redirect target) resolved against `base`.
fn resolve_location(base: &str, location: &str) -> Option<String> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return Some(location.to_string());
    }
    let (origin, _) = origin_of(base)?;
    if let Some(rest) = location.strip_prefix("//") {
        let scheme = &base[..base.find("://")?];
        return Some(format!("{scheme}://{rest}"));
    }
    if location.starts_with('/') {
        return Some(format!("{origin}{location}"));
    }
    let path = base[origin.len()..].split(['?', '#']).next().unwrap_or("/");
    let dir = &path[..path.rfind('/').map_or(0, |i| i + 1)];
    Some(format!("{origin}{dir}{location}"))
}

/// The header `name` of a response, as text.
fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// The hash a server states for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Stated {
    /// The SHA-256 of the content (Hugging Face LFS files: `X-Linked-Etag`).
    Sha256(String),
    /// The git blob id: SHA-1 of `blob <size>\0` + the content (Hugging Face files kept in git).
    GitSha1(String),
}

/// The hash inside an ETag value (`"<hex>"`, `W/"<hex>"`): 64 hex digits are a SHA-256, 40 a
/// git blob id; anything else is not a content hash.
fn etag_hash(value: &str) -> Option<Stated> {
    let hex = value.trim().trim_start_matches("W/").trim_matches('"');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        64 => Some(Stated::Sha256(hex.to_lowercase())),
        40 => Some(Stated::GitSha1(hex.to_lowercase())),
        _ => None,
    }
}

impl Client {
    fn new(endpoint: &str, token: Option<String>) -> Self {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            .user_agent(concat!("graphersal/", env!("CARGO_PKG_VERSION")))
            .build()
            .new_agent();
        Client {
            agent,
            endpoint_authority: origin_of(endpoint)
                .map(|(_, authority)| authority.to_string())
                .unwrap_or_default(),
            token,
        }
    }

    /// Downloads `url` (the file `name` of `repo`) into `dir/name`, following redirects
    /// itself so the stated hash of the first answer is seen and the token goes to the
    /// endpoint's host only.
    fn fetch(
        &self,
        url: &str,
        repo: &str,
        revision: &str,
        name: &'static str,
        dir: &Path,
    ) -> Result<Fetched, DownloadError> {
        let mut url = url.to_string();
        let mut stated: Option<Stated> = None;
        let mut expected_size: Option<u64> = None;
        let mut commit: Option<String> = None;
        for _ in 0..10 {
            let mut request = self.agent.get(&url);
            let same_host = origin_of(&url).is_some_and(|(_, a)| a == self.endpoint_authority);
            if same_host && let Some(token) = &self.token {
                request = request.header("Authorization", format!("Bearer {token}"));
            }
            let response = request
                .call()
                .map_err(|err| failed(format!("cannot fetch {name} of {repo}: {err}")))?;
            let status = response.status().as_u16();
            if stated.is_none() {
                stated = header(&response, "x-linked-etag").and_then(|v| etag_hash(&v));
            }
            if expected_size.is_none() {
                expected_size =
                    header(&response, "x-linked-size").and_then(|v| v.trim().parse().ok());
            }
            if commit.is_none() {
                commit = header(&response, "x-repo-commit");
            }
            match status {
                200 => {
                    if stated.is_none() {
                        stated = header(&response, "etag").and_then(|v| etag_hash(&v));
                    }
                    if expected_size.is_none() {
                        expected_size =
                            header(&response, "content-length").and_then(|v| v.trim().parse().ok());
                    }
                    let (sha256, size) =
                        self.stream(response, name, dir, expected_size, stated.as_ref())?;
                    return Ok(Fetched {
                        name,
                        size,
                        sha256,
                        commit,
                    });
                }
                301 | 302 | 303 | 307 | 308 => {
                    let location = header(&response, "location").ok_or_else(|| {
                        failed(format!("{name} of {repo}: a redirect without a Location"))
                    })?;
                    url = resolve_location(&url, &location).ok_or_else(|| {
                        failed(format!("{name} of {repo}: cannot follow the redirect"))
                    })?;
                }
                401 | 403 => {
                    return Err(failed(format!(
                        "{repo} refused the download of {name} (HTTP {status})\nHelp: the \
                         repository does not exist (Hugging Face answers 401 then), or it is \
                         gated or private and needs a Hugging Face token in HF_TOKEN (and the \
                         licence accepted on its page)"
                    )));
                }
                404 => {
                    return Err(failed(format!(
                        "{repo} at revision {revision} has no {name} (HTTP 404)\nHelp: check \
                         the repository name and the revision; a Model2Vec model holds \
                         config.json, tokenizer.json and model.safetensors at its root (for \
                         example minishlab/potion-base-8M)"
                    )));
                }
                _ => {
                    return Err(failed(format!(
                        "the download of {name} of {repo} failed: HTTP {status}"
                    )));
                }
            }
        }
        Err(failed(format!("{name} of {repo}: too many redirects")))
    }

    /// Streams the body into `dir/name.part`, checks size and hash, renames it to `dir/name`.
    fn stream(
        &self,
        response: ureq::http::Response<ureq::Body>,
        name: &'static str,
        dir: &Path,
        expected_size: Option<u64>,
        stated: Option<&Stated>,
    ) -> Result<(String, u64), DownloadError> {
        let part = dir.join(format!("{name}.part"));
        let mut file = fs::File::create(&part)
            .map_err(|err| failed(format!("cannot create {}: {err}", part.display())))?;
        let mut reader = response.into_body().into_reader();
        let mut digest = ring::digest::Context::new(&ring::digest::SHA256);
        // A git blob id needs the size up front: checked only when the server states it.
        let mut git = match (stated, expected_size) {
            (Some(Stated::GitSha1(_)), Some(size)) => {
                let mut context =
                    ring::digest::Context::new(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY);
                context.update(format!("blob {size}\0").as_bytes());
                Some(context)
            }
            _ => None,
        };
        let mut buffer = vec![0u8; 256 * 1024];
        let mut total: u64 = 0;
        let limit = expected_size.unwrap_or(MAX_FILE_BYTES);
        let progress = Progress::new(name, expected_size);
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|err| failed(format!("the download of {name} broke off: {err}")))?;
            if read == 0 {
                break;
            }
            total += read as u64;
            if total > limit {
                return Err(failed(format!(
                    "{name} is larger than {}",
                    human_bytes(limit)
                )));
            }
            digest.update(&buffer[..read]);
            if let Some(git) = git.as_mut() {
                git.update(&buffer[..read]);
            }
            file.write_all(&buffer[..read])
                .map_err(|err| failed(format!("cannot write {}: {err}", part.display())))?;
            progress.update(total);
        }
        file.sync_all()
            .map_err(|err| failed(format!("cannot write {}: {err}", part.display())))?;
        drop(file);
        progress.finish(total);
        if let Some(expected) = expected_size
            && total != expected
        {
            return Err(failed(format!(
                "the download of {name} broke off: {total} of {expected} bytes"
            )));
        }
        let sha256 = hex(digest.finish().as_ref());
        let mismatch = match stated {
            Some(Stated::Sha256(expected)) if *expected != sha256 => {
                Some(format!("SHA-256 {sha256}, the server states {expected}"))
            }
            Some(Stated::GitSha1(expected)) => git
                .map(|git| hex(git.finish().as_ref()))
                .filter(|actual| actual != expected)
                .map(|actual| format!("git blob id {actual}, the server states {expected}")),
            _ => None,
        };
        if let Some(mismatch) = mismatch {
            return Err(failed(format!(
                "{name}: the download is not the file the server describes ({mismatch}); \
                 nothing was kept"
            )));
        }
        fs::rename(&part, dir.join(name))
            .map_err(|err| failed(format!("cannot rename {}: {err}", part.display())))?;
        Ok((sha256, total))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The progress line of one file on stderr: rewritten in place on a terminal, else one line at
/// the end.
struct Progress {
    name: &'static str,
    expected: Option<u64>,
    terminal: bool,
    last: std::cell::Cell<Instant>,
}

impl Progress {
    fn new(name: &'static str, expected: Option<u64>) -> Self {
        Progress {
            name,
            expected,
            terminal: io::stderr().is_terminal(),
            last: std::cell::Cell::new(Instant::now()),
        }
    }

    fn text(&self, done: u64) -> String {
        match self.expected {
            Some(total) => format!(
                "  {}: {} / {}",
                self.name,
                human_bytes(done),
                human_bytes(total)
            ),
            None => format!("  {}: {}", self.name, human_bytes(done)),
        }
    }

    fn update(&self, done: u64) {
        if self.terminal && self.last.get().elapsed() >= Duration::from_millis(200) {
            self.last.set(Instant::now());
            crate::out::write_stderr(format_args!("\r{}\x1b[K", self.text(done)));
        }
    }

    fn finish(&self, done: u64) {
        if self.terminal {
            crate::out::write_stderr(format_args!("\r{}\x1b[K\n", self.text(done)));
        } else {
            errln!("{}", self.text(done));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_names() {
        for ok in ["minishlab/potion-base-8M", "bert-base-uncased", "a.b/c_d-e"] {
            assert!(is_repo_name(ok), "{ok}");
        }
        for bad in [
            "",
            "/",
            "a/",
            "/a",
            "a/b/c",
            "../x",
            "a/..",
            "a b/c",
            "a/.hidden",
        ] {
            assert!(!is_repo_name(bad), "{bad}");
        }
    }

    #[test]
    fn redirects_resolve() {
        let base = "https://huggingface.co/a/b/resolve/main/model.safetensors";
        assert_eq!(
            resolve_location(base, "https://cdn.example/x?y=1").as_deref(),
            Some("https://cdn.example/x?y=1")
        );
        assert_eq!(
            resolve_location(base, "/api/resolve-cache/models/a/b/c/config.json").as_deref(),
            Some("https://huggingface.co/api/resolve-cache/models/a/b/c/config.json")
        );
        assert_eq!(
            resolve_location(base, "other.json").as_deref(),
            Some("https://huggingface.co/a/b/resolve/main/other.json")
        );
        assert_eq!(
            resolve_location("http://127.0.0.1:9/x", "//cdn/y").as_deref(),
            Some("http://cdn/y")
        );
        assert_eq!(
            origin_of("http://127.0.0.1:8080/a/b"),
            Some(("http://127.0.0.1:8080", "127.0.0.1:8080"))
        );
    }

    #[test]
    fn etags() {
        let hex = "a".repeat(64);
        assert_eq!(
            etag_hash(&format!("\"{hex}\"")),
            Some(Stated::Sha256(hex.clone()))
        );
        assert_eq!(
            etag_hash(&format!("W/\"{hex}\"")),
            Some(Stated::Sha256(hex))
        );
        let sha1 = "b".repeat(40);
        assert_eq!(
            etag_hash(&format!("\"{sha1}\"")),
            Some(Stated::GitSha1(sha1))
        );
        assert_eq!(etag_hash("\"0123abcd\""), None);
        assert_eq!(etag_hash(&format!("\"{}\"", "z".repeat(64))), None);
    }

    #[test]
    fn sizes() {
        assert_eq!(human_bytes(10), "10 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(30 * 1024 * 1024), "30.0 MiB");
    }
}
