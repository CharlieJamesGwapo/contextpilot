use crate::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileContext {
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub excerpt: String,
    pub hash: String,
    pub tokens: u64,
    pub language: String,
    pub cached: bool,
    #[serde(default)]
    pub omitted: bool,
    #[serde(default)]
    pub note: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct RepositorySymbol {
    pub name: String,
    pub line: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IndexedFile {
    pub path: String,
    pub hash: String,
    pub language: String,
    pub symbols: Vec<RepositorySymbol>,
    pub imports: Vec<String>,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub cached: bool,
}
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RepositoryIndex {
    pub version: u32,
    pub files: Vec<IndexedFile>,
    pub cache_written: bool,
}
fn allowed(path: &Path) -> bool {
    path.components().all(|part| {
        let name = part.as_os_str().to_string_lossy().to_lowercase();
        !name.starts_with(".env")
            && !name.contains("secret")
            && !name.contains("credential")
            && ![
                "node_modules",
                "target",
                "dist",
                "out",
                "build",
                "coverage",
                ".next",
                ".git",
                ".contextpilot",
                "vendor",
                "__pycache__",
                ".aws",
                ".ssh",
                ".npmrc",
                ".netrc",
                ".pypirc",
                "id_rsa",
                "id_ed25519",
            ]
            .contains(&name.as_str())
            && ![".pem", ".key", ".sqlite", ".db", "lock", ".min.js", ".map"]
                .iter()
                .any(|suffix| name.ends_with(suffix))
    })
}
// Parse Rust syntax so an excerpt keeps the entire containing function, including
// trailing cleanup/error paths. Other languages retain the file until a parser is
// available, rather than pretending that a short line window is a complete scope.
fn source_scope(text: &str, language: &str, at: usize) -> Option<(usize, usize)> {
    if language != "rs" {
        return None;
    }
    use syn::{spanned::Spanned, visit::Visit};
    struct Scope {
        at: usize,
        best: Option<(usize, usize)>,
    }
    impl Scope {
        fn consider(&mut self, span: proc_macro2::Span) {
            let range = (span.start().line.saturating_sub(1), span.end().line);
            if range.0 <= self.at
                && self.at < range.1
                && self
                    .best
                    .is_none_or(|old| range.1 - range.0 < old.1 - old.0)
            {
                self.best = Some(range);
            }
        }
    }
    impl<'ast> Visit<'ast> for Scope {
        fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
            self.consider(node.span());
            syn::visit::visit_item_fn(self, node);
        }
        fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
            self.consider(node.span());
            syn::visit::visit_impl_item_fn(self, node);
        }
        fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
            self.consider(node.span());
            syn::visit::visit_trait_item_fn(self, node);
        }
    }
    let syntax = syn::parse_file(text).ok()?;
    let mut scope = Scope { at, best: None };
    scope.visit_file(&syntax);
    scope.best
}
pub fn project_root(start: &Path) -> PathBuf {
    let mut p = start.to_path_buf();
    loop {
        if p.join(".git").exists() || p.join(".contextpilot").exists() {
            return p;
        }
        if !p.pop() {
            return start.to_path_buf();
        }
    }
}
fn cache_path(root: &Path) -> Option<PathBuf> {
    let dir = root.join(".contextpilot");
    if let Ok(meta) = std::fs::symlink_metadata(&dir) {
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return None;
        }
    } else if std::fs::create_dir(&dir).is_err() {
        return None;
    }
    if crate::security::owner_only(&dir).is_err() {
        return None;
    }
    let file = dir.join("repository-index.json");
    if std::fs::symlink_metadata(&file).is_ok_and(|m| !m.is_file() || m.file_type().is_symlink()) {
        return None;
    }
    Some(file)
}
fn scan(root: &Path, exclusions: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut patterns = ignore::gitignore::GitignoreBuilder::new(root);
    for pattern in exclusions {
        patterns
            .add_line(None, pattern)
            .map_err(|_| "Invalid repository exclusion pattern")?;
    }
    let exclusions = patterns
        .build()
        .map_err(|_| "Invalid repository exclusion pattern")?;
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    let filter_root = root.to_path_buf();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .add_custom_ignore_filename(".contextpilotignore")
        .require_git(false)
        .follow_links(false)
        .filter_entry(move |entry| {
            entry.depth() == 0
                || (!exclusions
                    .matched_path_or_any_parents(
                        entry.path(),
                        entry.file_type().is_some_and(|t| t.is_dir()),
                    )
                    .is_ignore()
                    && allowed(
                        entry
                            .path()
                            .strip_prefix(&filter_root)
                            .unwrap_or(entry.path()),
                    ))
        })
        .build();
    for entry in walker {
        let entry = entry.map_err(|_| "Repository traversal failed")?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry
            .path()
            .canonicalize()
            .map_err(|_| "File unavailable")?;
        if !path.starts_with(root) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .map_err(|_| "Invalid root")?
            .to_string_lossy()
            .replace('\\', "/");
        if !allowed(Path::new(&rel)) {
            continue;
        }
        use std::io::Read;
        let mut text = String::new();
        let Ok(file) = std::fs::File::open(path) else {
            continue;
        };
        if file.take(512 * 1024 + 1).read_to_string(&mut text).is_err() || text.len() > 512 * 1024 {
            continue;
        }
        if !text.contains('\0') && !text.contains("-----BEGIN ") {
            total_bytes += text.len();
            if files.len() >= 20_000 || total_bytes > 128 * 1024 * 1024 {
                return Err(
                    "Repository exceeds 20,000 files or 128 MiB of text; add exclusions and retry"
                        .into(),
                );
            }
            files.push((rel, text));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}
/// Count every eligible full file, not the retrieval shortlist. Never returns
/// source contents or a silently partial directory total.
pub fn estimate_directory(
    root: &Path,
    exclusions: &[String],
    provider: &str,
    model: &str,
) -> Result<serde_json::Value, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "Repository root unavailable")?;
    if !root.is_dir() {
        return Err("Repository root must be a directory".into());
    }
    let files = scan(&root, exclusions)?;
    let mut tokens = 0u64;
    for (_, text) in &files {
        tokens = tokens
            .checked_add(estimate_for_model(text, provider, model).tokens)
            .ok_or("Directory token count overflow")?;
    }
    let basis = estimate_for_model("", provider, model);
    Ok(
        serde_json::json!({"tokens":tokens,"files_analyzed":files.len(),"measurement":basis.measurement,"method":basis.method,"uncertainty":basis.uncertainty,"scope":"eligible_full_files","coverage":"All eligible UTF-8 full files, subject to ignore/secret/generated exclusions and 512 KiB per-file limit; framing and multimodal tokens excluded."}),
    )
}
fn metadata(path: &str, text: &str) -> IndexedFile {
    let symbol = regex::Regex::new(r"(?:\b(?:fn|function|class|struct|enum|trait|interface|def|type|const|let|mod)\s+)([A-Za-z_][A-Za-z0-9_]*)").unwrap();
    let imports = regex::Regex::new(r#"(?:\b(?:from|import|use|mod)\s+[\"']?([A-Za-z_./:@][A-Za-z0-9_./:@-]*)|\brequire\s*\(\s*[\"']([^\"']+)|\bfrom\s*[\"']([^\"']+))"#).unwrap();
    let mut symbols = Vec::new();
    let mut refs = BTreeSet::new();
    for (line, text) in text.lines().enumerate() {
        let text = redact(text);
        for cap in symbol.captures_iter(&text) {
            symbols.push(RepositorySymbol {
                name: cap[1].into(),
                line: line + 1,
            });
        }
        for cap in imports.captures_iter(&text) {
            if let Some(value) = cap.get(1).or_else(|| cap.get(2)).or_else(|| cap.get(3)) {
                refs.insert(value.as_str().into());
            }
        }
    }
    IndexedFile {
        path: path.into(),
        hash: hash(text),
        language: Path::new(path)
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        symbols,
        imports: refs.into_iter().collect(),
        dependencies: Vec::new(),
        cached: false,
    }
}
fn normalized(path: &Path) -> String {
    let mut parts = Vec::new();
    for part in path.components() {
        match part {
            std::path::Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            std::path::Component::ParentDir => {
                parts.pop();
            }
            _ => {}
        }
    }
    parts.join("/")
}
fn build(root: &Path, source: &[(String, String)]) -> RepositoryIndex {
    let cache = cache_path(root);
    let old: Option<RepositoryIndex> = cache
        .as_ref()
        .and_then(|p| {
            std::fs::metadata(p)
                .ok()
                .filter(|m| m.len() <= 16 * 1024 * 1024)
                .and_then(|_| std::fs::read(p).ok())
        })
        .and_then(|b| serde_json::from_slice(&b).ok())
        .filter(|i: &RepositoryIndex| i.version == 1);
    let old: BTreeMap<_, _> = old
        .into_iter()
        .flat_map(|i| i.files)
        .map(|f| (f.path.clone(), f))
        .collect();
    let mut files: Vec<_> = source
        .iter()
        .map(|(path, text)| {
            // Hash every current file: timestamps and cached source text are never authoritative.
            if let Some(prior) = old.get(path).filter(|f| f.hash == hash(text)) {
                let mut f = prior.clone();
                f.cached = true;
                f
            } else {
                metadata(path, text)
            }
        })
        .collect();
    let paths: Vec<_> = files.iter().map(|f| f.path.clone()).collect();
    for file in &mut files {
        file.dependencies = paths
            .iter()
            .filter(|p| {
                **p != file.path
                    && file.imports.iter().any(|import| {
                        let module = import.trim_start_matches("crate::").replace("::", "/");
                        let stem = Path::new(p)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy();
                        let parent = Path::new(&file.path).parent().unwrap_or(Path::new(""));
                        let relative = normalized(&parent.join(&module));
                        let no_ext = p.rsplit_once('.').map_or(p.as_str(), |(s, _)| s);
                        no_ext == relative
                            || *p == &relative
                            || no_ext == format!("{relative}/index")
                            || no_ext == module
                            || (!module.contains('/') && stem == module)
                    })
            })
            .cloned()
            .collect();
    }
    let mut index = RepositoryIndex {
        version: 1,
        files,
        cache_written: false,
    };
    if let Some(path) = cache {
        // A fresh create_new temporary file prevents following attacker-controlled cache links.
        let temp = path.with_file_name(format!("index-{}.tmp", uuid::Uuid::new_v4()));
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
        {
            use std::io::Write;
            if crate::security::owner_only(&temp).is_err() {
                let _ = std::fs::remove_file(&temp);
                return index;
            }
            if serde_json::to_vec(&index)
                .ok()
                .is_some_and(|bytes| file.write_all(&bytes).is_ok())
                && std::fs::rename(&temp, &path).is_ok()
            {
                index.cache_written = true;
            }
            let _ = std::fs::remove_file(temp);
        }
    }
    index
}
pub fn index_repository(root: &Path) -> Result<RepositoryIndex, String> {
    index_repository_with_exclusions(root, &[])
}
pub fn index_repository_with_exclusions(
    root: &Path,
    exclusions: &[String],
) -> Result<RepositoryIndex, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "Project path unavailable")?;
    if !root.is_dir() {
        return Err("Project path is not a directory".into());
    }
    let source = scan(&root, exclusions)?;
    Ok(build(&root, &source))
}
pub fn retrieve(root: &Path, query: &str, max_files: usize) -> Result<Vec<FileContext>, String> {
    retrieve_with_exclusions(root, query, max_files, &[])
}
pub fn retrieve_with_exclusions(
    root: &Path,
    query: &str,
    max_files: usize,
    exclusions: &[String],
) -> Result<Vec<FileContext>, String> {
    let root = root
        .canonicalize()
        .map_err(|_| "Project path unavailable")?;
    if !root.is_dir() {
        return Err("Project path is not a directory".into());
    }
    if max_files == 0 {
        return Ok(Vec::new());
    }
    let source = scan(&root, exclusions)?;
    let index = build(&root, &source);
    let terms: BTreeSet<_> = query
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut ranked: Vec<_> = index
        .files
        .iter()
        .zip(&source)
        .map(|(file, (_, text))| {
            let score: usize = terms
                .iter()
                .map(|term| {
                    usize::from(file.path.to_lowercase().contains(term)) * 12
                        + file
                            .symbols
                            .iter()
                            .filter(|s| s.name.to_lowercase().contains(term))
                            .count()
                            * 20
                        + text
                            .lines()
                            .filter(|s| s.to_lowercase().contains(term))
                            .count()
                            .min(10)
                })
                .sum();
            (score, file, text)
        })
        .collect();
    let direct: BTreeSet<_> = ranked
        .iter()
        .filter(|(s, _, _)| *s > 0)
        .flat_map(|(_, f, _)| f.dependencies.iter().cloned())
        .collect();
    for (score, file, _) in &mut ranked {
        if direct.contains(&file.path) {
            *score += 3;
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.path.cmp(&b.1.path)));
    Ok(ranked
        .into_iter()
        .filter(|(score, _, _)| terms.is_empty() || *score > 0)
        .take(max_files.min(100))
        .map(|(_, file, text)| {
            let lines: Vec<_> = text.lines().collect();
            let at = lines
                .iter()
                .enumerate()
                .max_by_key(|(i, line)| {
                    (
                        terms
                            .iter()
                            .filter(|t| line.to_lowercase().contains(t.as_str()))
                            .count()
                            * 10
                            + usize::from(file.symbols.iter().any(|s| {
                                s.line == i + 1
                                    && terms.iter().any(|t| s.name.to_lowercase().contains(t))
                            })),
                        std::cmp::Reverse(*i),
                    )
                })
                .map_or(0, |(i, _)| i);
            let scope = source_scope(text, &file.language, at);
            let (start, end) = scope.unwrap_or((0, lines.len()));
            let excerpt = redact(&lines[start..end].join("\n"));
            FileContext {
                path: file.path.clone(),
                line_start: if lines.is_empty() { 0 } else { start + 1 },
                line_end: end,
                tokens: estimate(&excerpt).tokens,
                excerpt,
                hash: file.hash.clone(),
                language: file.language.clone(),
                cached: file.cached,
                omitted: false,
                note: Some(if scope.is_some() { "Complete containing Rust function retained; related imports/dependencies are indexed separately." } else { "Complete file retained because a safe containing syntax scope could not be established." }.into()),
            }
        })
        .collect())
}
