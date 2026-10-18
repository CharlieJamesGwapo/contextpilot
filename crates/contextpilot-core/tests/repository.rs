use contextpilot_core::repository::{index_repository, retrieve};
use std::{fs, path::PathBuf};
struct Repo(PathBuf);
impl Repo {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("contextpilot-repo-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn put(&self, p: &str, text: &str) {
        let path = self.0.join(p);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn current_content_invalidates_cache_and_deleted_entries() {
    let r = Repo::new();
    r.put("a.rs", "fn old_symbol() {}\n");
    let a = index_repository(&r.0).unwrap();
    assert!(!a.files[0].cached);
    assert!(a.cache_written);
    assert!(index_repository(&r.0).unwrap().files[0].cached);
    r.put("a.rs", "fn new_symbol() {}\n");
    let b = index_repository(&r.0).unwrap();
    assert!(!b.files[0].cached);
    assert_ne!(a.files[0].hash, b.files[0].hash);
    assert!(retrieve(&r.0, "old_symbol", 10).unwrap().is_empty());
    assert!(
        retrieve(&r.0, "new_symbol", 10).unwrap()[0]
            .excerpt
            .contains("new_symbol")
    );
    fs::remove_file(r.0.join("a.rs")).unwrap();
    assert!(index_repository(&r.0).unwrap().files.is_empty());
}
#[test]
fn ignores_secrets_generated_and_custom_patterns() {
    let r = Repo::new();
    r.put(".gitignore", "ignored.rs\n");
    r.put(".contextpilotignore", "custom.rs\n");
    for name in [
        "ignored.rs",
        "custom.rs",
        ".env",
        "credentials/data.rs",
        "secret.rs",
        "target/generated.rs",
        "dist/bundle.js",
        "id.key",
    ] {
        r.put(name, "fn forbidden() {}\n");
    }
    r.put("src/good.rs", "fn visible() {}\n");
    let i = index_repository(&r.0).unwrap();
    assert!(i.files.iter().any(|f| f.path == "src/good.rs"));
    assert!(!i.files.iter().any(|f| f.path.ends_with("ignored.rs")
        || f.path.ends_with("custom.rs")
        || f.path.contains("credential")
        || f.path.contains("secret")
        || f.path.starts_with("target/")
        || f.path.starts_with("dist/")
        || f.path == ".env"
        || f.path == "id.key"));
    r.put(".gitignore", "src/\n");
    assert!(
        !index_repository(&r.0)
            .unwrap()
            .files
            .iter()
            .any(|f| f.path == "src/good.rs")
    );
}
#[test]
fn symbol_ranking_relationships_and_targeted_lines() {
    let r = Repo::new();
    r.put("a.txt", "calculate\n");
    r.put(
        "src/engine.rs",
        &format!(
            "{}\nuse crate::helper;\nfn calculate() {{}}\n",
            "// filler\n".repeat(60)
        ),
    );
    r.put("helper.rs", "fn assist() {}\n");
    let i = index_repository(&r.0).unwrap();
    let engine = i.files.iter().find(|f| f.path == "src/engine.rs").unwrap();
    assert!(engine.symbols.iter().any(|s| s.name == "calculate"));
    assert_eq!(engine.dependencies, vec!["helper.rs"]);
    let result = retrieve(&r.0, "calculate", 3).unwrap();
    assert_eq!(result[0].path, "src/engine.rs");
    assert!(result[0].line_start > 50);
    assert!(result.iter().any(|f| f.path == "helper.rs"));
    assert!(retrieve(&r.0, "calculate", 0).unwrap().is_empty());
}
#[cfg(unix)]
#[test]
fn never_follows_source_or_cache_symlinks() {
    use std::os::unix::fs::symlink;
    let r = Repo::new();
    let outside = Repo::new();
    outside.put("private.rs", "fn escaped() {}\n");
    symlink(outside.0.join("private.rs"), r.0.join("link.rs")).unwrap();
    symlink(&outside.0, r.0.join(".contextpilot")).unwrap();
    r.put("safe.rs", "fn safe() {}\n");
    let i = index_repository(&r.0).unwrap();
    assert!(!i.cache_written);
    assert_eq!(i.files.len(), 1);
    assert!(!outside.0.join("repository-index.json").exists());
    assert!(retrieve(&r.0, "escaped", 10).unwrap().is_empty());
}
#[test]
fn corrupted_cache_recovers_without_stale_excerpts() {
    let r = Repo::new();
    r.put("a.py", "def current():\n    pass\n");
    index_repository(&r.0).unwrap();
    r.put(".contextpilot/repository-index.json", "broken");
    let i = index_repository(&r.0).unwrap();
    assert_eq!(i.files[0].symbols[0].name, "current");
}
#[test]
fn resolves_relative_imports_and_applies_configured_exclusions() {
    let r = Repo::new();
    r.put(
        "src/main.ts",
        "import { answer } from './utils';\nexport function run() {}\n",
    );
    r.put("src/utils.ts", "export const answer = 42;\n");
    r.put("private/data.ts", "export function run() {}\n");
    let i = index_repository(&r.0).unwrap();
    let main = i.files.iter().find(|f| f.path == "src/main.ts").unwrap();
    assert_eq!(main.dependencies, vec!["src/utils.ts"]);
    let results = contextpilot_core::repository::retrieve_with_exclusions(
        &r.0,
        "run",
        10,
        &["private/".into()],
    )
    .unwrap();
    assert!(results.iter().all(|f| !f.path.starts_with("private/")));
    assert!(results.iter().any(|f| f.path == "src/main.ts"));
}
#[test]
fn retrieval_includes_later_error_handling_in_the_same_function() {
    let root = std::env::temp_dir().join(format!("contextpilot-scope-{}", contextpilot_core::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut source = String::from("pub fn authenticate() {\n");
    source.push_str(&"    let value = 1;\n".repeat(35));
    source.push_str("    if failed { return Err(AuthenticationError); }\n}\n");
    std::fs::write(root.join("auth.rs"), source).unwrap();
    let files = contextpilot_core::retrieve(&root, "authenticate", 1).unwrap();
    assert!(files[0].excerpt.contains("AuthenticationError"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn directory_estimate_counts_all_files_beyond_retrieval_shortlist() {
    let r = Repo::new();
    for i in 0..20 {
        r.put(&format!("src/f{i}.rs"), "fn eligible() {}\n");
    }
    r.put(".env", "private");
    let result = contextpilot_core::estimate_directory(&r.0, &[], "local", "").unwrap();
    assert_eq!(result["files_analyzed"], 20);
    assert_eq!(
        result["tokens"],
        contextpilot_core::estimate("fn eligible() {}\n").tokens * 20
    );
    assert_eq!(result["scope"], "eligible_full_files");
    assert!(result.get("text").is_none());
}

#[test]
fn bare_file_exclusions_apply_to_directory_estimates_and_retrieval() {
    let r = Repo::new();
    r.put("excluded.rs", "fn excluded() {}\n");
    r.put("nested/excluded.rs", "fn excluded() {}\n");
    r.put("module.rs", "fn included() {}\n");
    let exclusions = vec!["excluded.rs".into()];
    let result = contextpilot_core::estimate_directory(&r.0, &exclusions, "local", "").unwrap();
    assert_eq!(result["files_analyzed"], 1);
    let files = contextpilot_core::retrieve_with_exclusions(&r.0, "", 100, &exclusions).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "module.rs");
}
