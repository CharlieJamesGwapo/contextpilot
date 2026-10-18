use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use contextpilot_app::{
    api::{self, AppState},
    storage::Store,
};
use contextpilot_core::*;
use contextpilot_providers::{Provider, ProviderClient};
use serde_json::json;
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Parser)]
#[command(
    name = "contextpilot",
    version,
    about = "Local AI context optimization and recoverable sessions",
    after_help = "Examples:\n  contextpilot doctor\n  contextpilot init\n  contextpilot tokens task.md\n  contextpilot optimize --stdin\n  contextpilot session create 'Fix login' --requirement 'Test auth'\n  contextpilot dashboard\nAll token counts are estimates unless marked provider_reported."
)]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "CONTEXTPILOT_DATA_DIR",
        default_value = ".contextpilot"
    )]
    data_dir: PathBuf,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Safely summarize a captured tool output; never execute its contents.
    ToolOutput {
        file: PathBuf,
        #[arg(long)]
        exit_status: Option<i32>,
        /// Save the complete redacted output to a NEW private file.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Export machine-readable contracts derived from Rust serde types.
    Schema,
    /// Register/list projects, inspect repository indexes and session associations.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Import normalized historical usage with provenance and stable event identity.
    ImportUsage {
        file: PathBuf,
    },
    /// Record quality/retry evidence from an existing controlled evaluation.
    Evaluate {
        file: PathBuf,
    },
    /// List or register user-supplied model metadata.
    Models {
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Export configurable agent efficiency guidance without touching external sessions.
    Instructions {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Export a continuation and structured checkpoint to a NEW handoff directory.
    Handoff {
        #[arg(long)]
        session: String,
        output: PathBuf,
    },
    Version,
    /// Check local storage, configuration and provider capabilities.
    Doctor,
    /// Create project metadata without overwriting existing files.
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Estimate a file, text from stdin, or ignored-aware directory context.
    Tokens {
        path: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    /// Conservative local optimization; output files must be new.
    Optimize {
        path: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long, default_value = "quick")]
        mode: String,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        allow_external: bool,
    },
    Status,
    Report {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Show or update validated settings from a JSON file.
    Config {
        #[arg(long)]
        file: Option<PathBuf>,
    },
    #[command(subcommand)]
    Session(SessionCommand),
    #[command(subcommand)]
    Context(ContextCommand),
    /// Preview by default. --commit requires the preview revision.
    Compact {
        #[arg(long)]
        session: String,
        #[arg(long)]
        commit: bool,
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        revision: Option<u64>,
    },
    #[command(subcommand)]
    Checkpoint(CheckpointCommand),
    #[command(subcommand)]
    Providers(ProviderCommand),
    /// Retrieve bounded source excerpts, honoring ignore files and secret exclusions.
    Retrieve {
        query: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 8)]
        max_files: usize,
    },
    /// Start protected loopback API and serve a prebuilt dashboard.
    Dashboard {
        #[arg(long, default_value_t = 4317)]
        port: u16,
        #[arg(long)]
        assets: Option<PathBuf>,
    },
    /// Export local data (includes redacted owned session content).
    Export {
        output: PathBuf,
    },
    /// Make a SQLite-consistent backup to a new file.
    Backup {
        output: PathBuf,
    },
    /// Restore backup into a NEW data directory. Existing databases are never replaced.
    RestoreBackup {
        input: PathBuf,
        output_dir: PathBuf,
    },
    /// Delete usage metadata with explicit confirmation flag.
    ClearUsage {
        #[arg(long)]
        yes: bool,
    },
    /// Save a user supplied pricing record; no current prices are bundled.
    Price {
        file: PathBuf,
    },
}
#[derive(Subcommand)]
enum ProjectCommand {
    Config {
        id: String,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    List,
    Add {
        root: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Index {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    Link {
        session: String,
        project: String,
    },
}
#[derive(Subcommand)]
enum SessionCommand {
    /// Attach hash-validated project excerpts once; stale replacements keep recovery snapshots.
    Retrieve {
        id: String,
        query: String,
        #[arg(long, default_value_t = 8)]
        max_files: usize,
    },
    Create {
        objective: String,
        #[arg(long)]
        requirement: Vec<String>,
        #[arg(long)]
        constraint: Vec<String>,
        #[arg(long)]
        issue: Vec<String>,
        #[arg(long)]
        next: Vec<String>,
        #[arg(long)]
        limit: Option<u64>,
    },
    List,
    Show {
        id: String,
    },
    Add {
        id: String,
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "context")]
        category: String,
    },
    /// Resume an owned session using an explicitly chosen external provider/model.
    Run {
        id: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        model: String,
        #[arg(long, default_value_t = 2048)]
        max_output: u32,
        #[arg(long)]
        allow_external: bool,
    },
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand)]
enum ContextCommand {
    Status {
        #[arg(long)]
        session: String,
    },
}
#[derive(Subcommand)]
enum CheckpointCommand {
    Create {
        #[arg(long)]
        session: String,
    },
    List,
    Restore {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    Export {
        id: String,
        output: PathBuf,
    },
}
#[derive(Subcommand)]
enum ProviderCommand {
    /// Store a key from stdin in the native OS vault, or remove the vault entry.
    Credentials {
        provider: String,
        #[arg(long, conflicts_with = "remove")]
        stdin: bool,
        #[arg(long, conflicts_with = "stdin")]
        remove: bool,
    },
    /// Ask a supported provider to count text (requires explicit network opt-in).
    Count {
        provider: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        allow_external: bool,
    },
    List,
    Status,
    Configure,
}
fn emit(value: impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
fn directory_config(db: &Store, root: &std::path::Path) -> Result<Config> {
    let canonical = root.canonicalize()?;
    for project in db.list("projects")? {
        if project["root"]
            .as_str()
            .is_some_and(|p| std::path::Path::new(p) == canonical)
            && let Some(id) = project["id"].as_str()
        {
            return db.effective_project_config(id);
        }
    }
    db.config()
}

fn new_file(path: &std::path::Path, text: &str) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .context("Output already exists or cannot be created")?;
    contextpilot_app::security::owner_only(path)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}
fn input(path: Option<PathBuf>, stdin: bool) -> Result<String> {
    if stdin {
        let mut text = String::new();
        std::io::stdin().take(1_048_577).read_to_string(&mut text)?;
        if text.len() > 1_048_576 {
            bail!("Input exceeds 1 MiB");
        }
        return Ok(text);
    }
    let p = path.context("Provide a path or --stdin")?;
    if std::fs::metadata(&p)?.len() > 1_048_576 {
        bail!("Input exceeds 1 MiB");
    }
    std::fs::read_to_string(p).context("Input must be a readable UTF-8 text file")
}
fn parse_provider(p: &str) -> Result<Provider> {
    match p {
        "openai" | "open_ai" => Ok(Provider::OpenAi),
        "anthropic" => Ok(Provider::Anthropic),
        _ => bail!("Supported providers: openai, anthropic"),
    }
}
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("ContextPilot: {}", redact(&e.to_string()));
        std::process::exit(1)
    }
}
async fn run() -> Result<()> {
    let cli = Cli::parse();
    // Commands that do not need persistent storage.
    match &cli.command {
        Command::Schema => return emit(contextpilot_app::contracts::schemas()),
        Command::Version => return emit(json!({"version":env!("CARGO_PKG_VERSION")})),
        Command::RestoreBackup { input, output_dir } => {
            let dbpath = output_dir.join("state.sqlite");
            if dbpath.exists() {
                bail!("Restore destination already contains a database");
            }
            Store::restore_backup(input, &dbpath)?;
            return emit(json!({"restored_to":dbpath}));
        }
        _ => {}
    }
    let dbpath = cli.data_dir.join("state.sqlite");
    let mut db = Store::open(&dbpath)?;
    match cli.command {
        Command::Tokens {
            path,
            stdin,
            provider,
            model,
        } => {
            let config = db.config()?;
            if path.as_ref().is_some_and(|p| p.is_dir()) && !stdin {
                let root = path.as_ref().unwrap();
                let config = directory_config(&db, root)?;
                emit(
                    estimate_directory(
                        root,
                        &config.project_exclusions,
                        provider.as_deref().unwrap_or(&config.tokenizer_provider),
                        model.as_deref().unwrap_or(&config.tokenizer_model),
                    )
                    .map_err(anyhow::Error::msg)?,
                )
            } else {
                let text = input(path, stdin)?;
                emit(estimate_for_model(
                    &text,
                    provider.as_deref().unwrap_or(&config.tokenizer_provider),
                    model.as_deref().unwrap_or(&config.tokenizer_model),
                ))
            }
        }
        Command::Retrieve {
            query,
            root,
            max_files,
        } => {
            let c = directory_config(&db, &root)?;
            let mut files = retrieve_with_exclusions(
                &root,
                &query,
                max_files.min(c.profile.max_files),
                &c.project_exclusions,
            )
            .map_err(anyhow::Error::msg)?;
            let mut used = 0u64;
            for file in &mut files {
                file.tokens =
                    estimate_for_model(&file.excerpt, &c.tokenizer_provider, &c.tokenizer_model)
                        .tokens;
                if used.saturating_add(file.tokens) > c.profile.max_file_tokens {
                    file.omitted = true;
                    file.note = Some(format!(
                        "Excerpt omitted: configured retrieval token budget {} would be exceeded. Increase the profile budget to inspect this source; its path and line range are preserved.",
                        c.profile.max_file_tokens
                    ));
                    file.excerpt.clear();
                    file.tokens = 0;
                } else {
                    used = used.saturating_add(file.tokens);
                }
            }
            emit(files)
        }
        Command::Project(command) => match command {
            ProjectCommand::Config { id, file } => {
                if let Some(file) = file {
                    let patch = serde_json::from_str(&input(Some(file), false)?)?;
                    emit(db.set_project_config(&id, patch)?)
                } else {
                    emit(db.effective_project_config(&id)?)
                }
            }
            ProjectCommand::List => emit(db.list("projects")?),
            ProjectCommand::Add { root, name } => emit(db.register_project(&root, name)?),
            ProjectCommand::Index { root } => {
                let c = directory_config(&db, &root)?;
                emit(
                    index_repository_with_exclusions(&root, &c.project_exclusions)
                        .map_err(anyhow::Error::msg)?,
                )
            }
            ProjectCommand::Link { session, project } => {
                emit(db.link_project(&session, Some(project))?)
            }
        },
        Command::ImportUsage { file } => {
            let r: contextpilot_app::storage_extended::UsageImport =
                serde_json::from_str(&input(Some(file), false)?)?;
            emit(db.import_usage(&r)?)
        }
        Command::Evaluate { file } => {
            let r: serde_json::Value = serde_json::from_str(&input(Some(file), false)?)?;
            emit(db.record_outcome(r)?)
        }
        Command::Models { file } => {
            if let Some(file) = file {
                let m: contextpilot_app::storage_extended::ModelMetadata =
                    serde_json::from_str(&input(Some(file), false)?)?;
                db.set_model(&m)?;
            }
            emit(db.list("models")?)
        }
        Command::ToolOutput {
            file,
            exit_status,
            output,
        } => {
            let text = input(Some(file), false)?;
            let mut report = tool_output_report(
                &text,
                db.config()?.profile.max_tool_output_chars,
                exit_status,
            );
            if let Some(output) = output {
                new_file(&output, &redact(&text))?;
                report.complete_output_path = Some(output.to_string_lossy().into());
            }
            emit(report)
        }
        Command::Instructions { output } => {
            let text = db.config()?.profile.instructions();
            if let Some(output) = output {
                new_file(&output, &text)?;
                emit(json!({"exported":output}))
            } else {
                emit(json!({"instructions":text}))
            }
        }
        Command::Handoff { session, output } => {
            if output.exists() {
                bail!("Handoff directory exists; choose a new path");
            }
            let p = db.preview_session(&session)?;
            std::fs::create_dir_all(&output)?;
            new_file(
                &output.join("checkpoint.json"),
                &serde_json::to_string_pretty(&p.checkpoint)?,
            )?;
            new_file(&output.join("continuation.md"), &p.checkpoint.continuation)?;
            new_file(
                &output.join("AGENT-INSTRUCTIONS.md"),
                &db.config()?.profile.instructions(),
            )?;
            emit(json!({"exported":output,"external_session_modified":false,"schema_version":1}))
        }
        Command::Providers(ProviderCommand::Credentials {
            provider,
            stdin,
            remove,
        }) => {
            let p = parse_provider(&provider)?;
            if remove {
                contextpilot_providers::credentials::remove(p)?;
                emit(json!({"removed":true,"environment_unchanged":true}))
            } else if stdin {
                use std::io::IsTerminal;
                if std::io::stdin().is_terminal() {
                    bail!(
                        "Pipe a credential through stdin to avoid terminal echo; do not pass keys as arguments"
                    );
                }
                let key = input(None, true)?;
                contextpilot_providers::credentials::set(p, key.trim_end_matches(['\r', '\n']))?;
                emit(json!({"stored":"native_os_vault","provider":provider}))
            } else {
                bail!("Use --stdin to store a key, or --remove to remove it");
            }
        }
        Command::Providers(ProviderCommand::Count {
            provider,
            model,
            file,
            allow_external,
        }) => {
            if !allow_external || !db.config()?.external_enabled {
                bail!("Provider counting requires external_enabled and --allow-external");
            }
            let text = input(Some(file), false)?;
            emit(
                ProviderClient::new(Duration::from_secs(60))?
                    .count_tokens(parse_provider(&provider)?, &model, &text, true)
                    .await?,
            )
        }
        Command::Doctor => emit(
            json!({"version":env!("CARGO_PKG_VERSION"),"storage":"writable","database":dbpath,"config":db.config()?,"providers":api::providers(),"tokenizer":db.estimate_text("")?,"external_session_mutation":false}),
        ),
        Command::Init { path } => {
            let root = path.canonicalize()?;
            let dir = root.join(".contextpilot");
            std::fs::create_dir_all(&dir)?;
            let config = dir.join("project.json");
            new_file(
                &config,
                &serde_json::to_string_pretty(
                    &json!({"schema_version":1,"root":root,"exclusions":[".env*","*.pem","*.key","node_modules","target"]}),
                )?,
            )?;
            let key = id();
            db.put("projects",&key,&json!({"id":key,"name":root.file_name().unwrap_or_default().to_string_lossy(),"root":root,"created_at":now()}))?;
            new_file(
                &dir.join("INSTRUCTIONS.md"),
                &db.config()?.profile.instructions(),
            )?;
            emit(json!({"initialized":root,"configuration":config}))
        }
        Command::Optimize {
            path,
            stdin,
            mode,
            output,
            provider,
            model,
            allow_external,
        } => {
            if output.as_ref().is_some_and(|p| p.exists()) {
                bail!("Output exists; choose a new file");
            }
            let source = input(path, stdin)?;
            let result = if mode == "quick" {
                let result = db.optimize_text(&source)?;
                db.record_optimization(&result)?;
                result
            } else if mode == "deep" {
                if !allow_external || !db.config()?.external_enabled {
                    bail!("Deep Optimize requires external_enabled=true and --allow-external");
                }
                if redact(&source) != source {
                    bail!("Potential secrets detected; redact before external optimization");
                }
                let provider =
                    parse_provider(&provider.context("--provider required in deep mode")?)?;
                let model = model.context("--model required in deep mode")?;
                let generated = ProviderClient::new(Duration::from_secs(60))?
                    .generate(provider, &model, &deep_instruction(&source), 2048, true)
                    .await?;
                let result = deep_optimization(&source, &generated.text);
                db.record_external_optimization(&result, generated.usage.as_ref())?;
                db.record_prompt_log(&source, &result.optimized)?;
                if let Some(usage) = &generated.usage {
                    db.record_provider_usage(&format!("{provider:?}"), &model, usage)?;
                }
                result
            } else {
                bail!("Supported modes: quick, deep")
            };
            if let Some(path) = output {
                new_file(&path, &result.optimized)?;
            }
            emit(result)
        }
        Command::Status => {
            let mut status = db.status()?;
            status["providers"] = api::providers();
            emit(status)
        }
        Command::Report { output } => {
            let report = db.analytics()?;
            let full = serde_json::to_string_pretty(&report)?;
            if let Some(path) = output {
                new_file(&path, &full)?;
                emit(json!({"exported":path}))
            } else if full.len() > db.config()?.profile.max_report_chars {
                emit(
                    json!({"events":report["events"],"input_tokens":report["input_tokens"],"output_tokens":report["output_tokens"],"estimated_costs_by_currency":report["estimated_costs_by_currency"],"detail_omitted":true,"reason":"Configured report length exceeded; use report --output NEW_FILE for complete data"}),
                )
            } else {
                emit(report)
            }
        }
        Command::Config { file } => {
            if let Some(file) = file {
                let c: Config = serde_json::from_str(&input(Some(file), false)?)?;
                db.set_config(&c)?;
            }
            emit(db.config()?)
        }
        Command::Session(command) => match command {
            SessionCommand::Retrieve {
                id,
                query,
                max_files,
            } => emit(db.attach_project_context(&id, &query, max_files)?),
            SessionCommand::Create {
                objective,
                requirement,
                constraint,
                issue,
                next,
                limit,
            } => {
                let session = Session::new(objective, requirement, constraint, issue, next, limit)
                    .map_err(anyhow::Error::msg)?;
                emit(db.create_session(&session)?)
            }
            SessionCommand::List => emit(db.list("sessions")?),
            SessionCommand::Show { id } => emit(db.session(&id)?),
            SessionCommand::Add { id, text, category } => {
                emit(db.add_message(&id, text, category)?)
            }
            SessionCommand::Delete { id, yes } => {
                if !yes {
                    bail!("Deletion requires --yes; export first if recovery is needed");
                }
                db.delete_session(&id)?;
                emit(json!({"deleted":true}))
            }
            SessionCommand::Run {
                id,
                provider,
                model,
                max_output,
                allow_external,
            } => {
                if !allow_external || !db.config()?.external_enabled {
                    bail!(
                        "Requires config external_enabled=true and --allow-external; session content is sent to the chosen provider"
                    );
                }
                if max_output > 32768 {
                    bail!("Output limit exceeds 32768");
                }
                let session = db.session(&id)?;
                if !db.session_config(&session)?.external_enabled {
                    bail!("External calls disabled for this project");
                }
                let b = db.context_budget(&session)?;
                if b.state == "emergency" {
                    bail!("Context budget exceeded; compact first");
                }
                let p = parse_provider(&provider)?;
                let mut result = ProviderClient::new(Duration::from_secs(60))?
                    .generate(p, &model, &session.text(), max_output, true)
                    .await?;
                if let Some(u) = &result.usage {
                    db.record_session_usage(&id, &provider, &model, u)?;
                }
                result.text = redact(&result.text);
                let committed = db
                    .add_message_at_revision(
                        &id,
                        result.text.clone(),
                        "assistant".into(),
                        Some(session.revision),
                    )
                    .is_ok();
                emit(
                    json!({"generation":result,"committed":committed,"warning":if committed{None}else{Some("Session changed or append failed; response returned without changing session")}}),
                )
            }
        },
        Command::Context(ContextCommand::Status { session }) => {
            emit(db.context_budget(&db.session(&session)?)?)
        }
        Command::Compact {
            session,
            commit,
            preview: _,
            revision,
        } => {
            if commit {
                emit(db.compact(
                    &session,
                    revision.context("--revision from preview required")?,
                )?)
            } else {
                emit(db.preview_session(&session)?)
            }
        }
        Command::Checkpoint(command) => match command {
            CheckpointCommand::Create { session } => {
                let s = db.session(&session)?;
                emit(db.compact(&session, s.revision)?.checkpoint)
            }
            CheckpointCommand::List => emit(db.list("checkpoints")?),
            CheckpointCommand::Restore { id, yes } => {
                if !yes {
                    bail!(
                        "Restore requires --yes; current state will be checkpointed automatically"
                    );
                }
                emit(db.restore(&id)?)
            }
            CheckpointCommand::Export { id, output } => {
                let checkpoint = db.checkpoint(&id)?;
                new_file(&output, &checkpoint.continuation)?;
                emit(json!({"exported":output}))
            }
        },
        Command::Providers(ProviderCommand::Configure) => emit(
            json!({"credentials":"Set OPENAI_API_KEY or ANTHROPIC_API_KEY in the server/CLI environment. Never enter keys into the dashboard.","consent":"Enable external_enabled in config and explicitly consent per request.","providers":api::providers()}),
        ),
        Command::Providers(_) => emit(api::providers()),
        Command::Price { file } => {
            let p: Price = serde_json::from_str(&input(Some(file), false)?)?;
            p.validate().map_err(anyhow::Error::msg)?;
            if p.verified {
                bail!("User supplied pricing must set verified=false");
            }
            db.put(
                "prices",
                &format!("{}:{}:{}", p.provider, p.model, p.effective_date),
                &p,
            )?;
            emit(p)
        }
        Command::Export { output } => {
            new_file(&output, &serde_json::to_string_pretty(&db.status()?)?)?;
            emit(json!({"exported":output}))
        }
        Command::Backup { output } => {
            db.backup(&output)?;
            emit(json!({"backup":output}))
        }
        Command::ClearUsage { yes } => {
            if !yes {
                bail!("Requires --yes");
            }
            db.clear_usage()?;
            emit(json!({"deleted":true}))
        }
        Command::Dashboard { port, assets } => {
            let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
                .await
                .context("Port unavailable; choose another --port")?;
            let port = listener.local_addr()?.port();
            let token = format!("{}{}", id(), id());
            let token_path = cli.data_dir.join(format!("dashboard-{port}.token"));
            if token_path.exists() {
                bail!(
                    "Token file already exists; choose another port or remove the old file after stopping its server"
                );
            }
            new_file(&token_path, &token)?;
            let static_dir = assets.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/dashboard/out")
            });
            if !static_dir.join("index.html").exists() {
                eprintln!(
                    "Dashboard assets missing. Run npm install && npm run build in apps/dashboard, or pass --assets. API remains available."
                );
            }
            eprintln!(
                "ContextPilot: http://127.0.0.1:{port}\nLocal API token stored in {} (paste it into Connect). Keep this file private.",
                token_path.display()
            );
            let state = AppState {
                store: Arc::new(Mutex::new(db)),
                token,
                port,
            };
            let result = axum::serve(listener, api::router(state, static_dir))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await;
            let _ = std::fs::remove_file(token_path);
            result?;
            Ok(())
        }
        _ => Ok(()),
    }
}
