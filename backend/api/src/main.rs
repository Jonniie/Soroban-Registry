mod commands;
mod config;
mod export;
mod import;
mod manifest;
mod multisig;
mod patch;
mod profiler;
mod test_framework;
mod wizard;
mod aggregation;
mod analytics;
mod audit_handlers;
mod audit_routes;
mod benchmark_engine;
mod benchmark_handlers;
mod benchmark_routes;
mod cache;
mod cache_benchmark;
mod checklist;
mod contract_history_handlers;
mod contract_history_routes;
mod detector;
mod error;
mod handlers;
mod models;
mod multisig_handlers;
mod multisig_routes;
mod popularity;
mod rate_limit;
mod routes;
mod state;
mod trust;
mod health_monitor;
mod migration_cli;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use patch::Severity;

/// Soroban Registry CLI — discover, publish, verify, and deploy Soroban contracts
#[derive(Debug, Parser)]
#[command(name = "soroban-registry", version, about, long_about = None)]
pub struct Cli {
    /// Registry API URL
    #[arg(
        long,
        env = "SOROBAN_REGISTRY_API_URL",
        default_value = "http://localhost:3001"
    )]
    pub api_url: String,

    /// Stellar network to use (mainnet | testnet | futurenet). Defaults to mainnet.
    #[arg(long, global = true)]
    pub network: Option<String>,

    /// Enable verbose output (shows HTTP requests, responses, and debug info)
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Search for contracts in the registry
    Search {
        /// Search query text
        #[arg(long)]
        query: String,

        /// Filter by category (e.g. dex, token, nft)
        #[arg(long)]
        category: Option<String>,

        /// Only show verified contracts
        #[arg(long)]
        verified_only: bool,

        /// Maximum number of results to return
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Output format
        #[arg(long, value_enum, default_value_t = SearchFormat::Table)]
        format: SearchFormat,
    },

    /// Get detailed information about a contract
    Info {
        /// Contract ID to look up
        contract_id: String,
    },

    /// Publish a new contract to the registry
    Publish {
        /// On-chain contract ID
        #[arg(long)]
        contract_id: String,

        /// Human-readable contract name
        #[arg(long)]
        name: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,

        /// Contract category (e.g. token, defi, nft)
        #[arg(long)]
        category: Option<String>,
@@ -195,50 +209,56 @@ pub enum Commands {
    },

    /// Run integration tests
    Test {
        /// Path to test file (YAML or JSON)
        test_file: String,

        /// Path to contract directory or file
        #[arg(long)]
        contract_path: Option<String>,

        /// Output JUnit XML report
        #[arg(long)]
        junit: Option<String>,

        /// Show coverage report
        #[arg(long, default_value = "true")]
        coverage: bool,
    // Spawn background popularity scoring job (runs hourly)
    popularity::spawn_popularity_task(pool.clone());
    // Spawn the hourly analytics aggregation background task
    aggregation::spawn_aggregation_task(pool.clone());

        /// Verbose output
        #[arg(long, short)]
        verbose: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SearchFormat {
    Json,
    Table,
}

/// Sub-commands for the `multisig` group
#[derive(Debug, Subcommand)]
pub enum MultisigCommands {
    /// Create a new multi-sig policy (defines signers and required threshold)
    CreatePolicy {
        #[arg(long)]
        name: String,
        #[arg(long)]
        threshold: u32,
        #[arg(long)]
        signers: String,
        #[arg(long)]
        expiry_secs: Option<u32>,
        #[arg(long)]
        created_by: String,
    },

    /// Create an unsigned deployment proposal
    CreateProposal {
        #[arg(long)]
        contract_name: String,
        #[arg(long)]
        contract_id: String,
        #[arg(long)]
        wasm_hash: String,
@@ -304,195 +324,309 @@ pub enum PatchCommands {
    Deps {
        #[command(subcommand)]
        command: DepsCommands,
    },
}

#[derive(Subcommand)]
enum DepsCommands {
    /// List dependencies for a contract
    List {
        /// Contract ID
        contract_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── Initialise logger ─────────────────────────────────────────────────────
    // --verbose / -v  →  DEBUG level (shows HTTP calls, payloads, timing)
    // default         →  WARN level  (only errors and warnings)
    let log_level = if cli.verbose { "debug" } else { "warn" };
    env_logger::Builder::new()
        .parse_filters(log_level)
        .format_timestamp(None) // no timestamps in CLI output
        .format_module_path(cli.verbose) // show module path only in verbose
        .init();

    log::debug!("Verbose mode enabled");
    log::debug!("API URL: {}", cli.api_url);

    // ── Resolve network ───────────────────────────────────────────────────────
    let network = config::resolve_network(cli.network)?;
    log::debug!("Network: {:?}", network);

    match cli.command {
        Commands::Search {
            query,
            category,
            verified_only,
            limit,
            format,
        } => {
            log::debug!(
                "Command: search | query={:?} category={:?} verified_only={} limit={} format={:?}",
                query,
                category,
                verified_only,
                limit,
                format
            );
            commands::search(
                &cli.api_url,
                &query,
                network,
                category.as_deref(),
                verified_only,
                limit,
                matches!(format, SearchFormat::Json),
            )
            .await?;
        }
        Commands::Info { contract_id } => {
            log::debug!("Command: info | contract_id={}", contract_id);
            commands::info(&cli.api_url, &contract_id, network).await?;
        }
        Commands::Publish {
            contract_id,
            name,
            description,
            category,
            tags,
            publisher,
        } => {
            let tags_vec = tags
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            log::debug!(
                "Command: publish | contract_id={} name={} tags={:?}",
                contract_id,
                name,
                tags_vec
            );
            commands::publish(
                &cli.api_url,
                &contract_id,
                &name,
                description.as_deref(),
                network,
                category.as_deref(),
                tags_vec,
                &publisher,
            )
            .await?;
        }
        Commands::List { limit } => {
            log::debug!("Command: list | limit={}", limit);
            commands::list(&cli.api_url, limit, network).await?;
        }
        Commands::Migrate {
            contract_id,
            wasm,
            simulate_fail,
            dry_run,
        } => {
            log::debug!(
                "Command: migrate | contract_id={} wasm={} dry_run={}",
                contract_id,
                wasm,
                dry_run
            );
            commands::migrate(&cli.api_url, &contract_id, &wasm, simulate_fail, dry_run).await?;
        }
        Commands::Export {
            id,
            output,
            contract_dir,
        } => {
            log::debug!("Command: export | id={} output={}", id, output);
            commands::export(&cli.api_url, &id, &output, &contract_dir).await?;
        }
        Commands::Import {
            archive,
            output_dir,
        } => {
            log::debug!(
                "Command: import | archive={} output_dir={}",
                archive,
                output_dir
            );
            commands::import(&cli.api_url, &archive, network, &output_dir).await?;
        }
        Commands::Doc {
            contract_path,
            output,
        } => {
            log::debug!(
                "Command: doc | contract_path={} output={}",
                contract_path,
                output
            );
            commands::doc(&contract_path, &output)?;
        }
        Commands::Wizard {} => {
            log::debug!("Command: wizard");
            wizard::run(&cli.api_url).await?;
        }
        Commands::History { search, limit } => {
            log::debug!("Command: history | search={:?} limit={}", search, limit);
            wizard::show_history(search.as_deref(), limit)?;
        }
        Commands::Patch { action } => match action {
            PatchCommands::Create {
                version,
                hash,
                severity,
                rollout,
            } => {
                let sev = severity.parse::<Severity>()?;
                log::debug!(
                    "Command: patch create | version={} rollout={}",
                    version,
                    rollout
                );
                commands::patch_create(&cli.api_url, &version, &hash, sev, rollout).await?;
            }
            PatchCommands::Notify { patch_id } => {
                log::debug!("Command: patch notify | patch_id={}", patch_id);
                commands::patch_notify(&cli.api_url, &patch_id).await?;
            }
            PatchCommands::Apply {
                contract_id,
                patch_id,
            } => {
                log::debug!(
                    "Command: patch apply | contract_id={} patch_id={}",
                    contract_id,
                    patch_id
                );
                commands::patch_apply(&cli.api_url, &contract_id, &patch_id).await?;
            }
        },
        Commands::Multisig { action } => match action {
            MultisigCommands::CreatePolicy {
                name,
                threshold,
                signers,
                expiry_secs,
                created_by,
            } => {
                let signer_vec: Vec<String> =
                    signers.split(',').map(|s| s.trim().to_string()).collect();
                log::debug!(
                    "Command: multisig create-policy | name={} threshold={} signers={:?}",
                    name,
                    threshold,
                    signer_vec
                );
                multisig::create_policy(
                    &cli.api_url,
                    &name,
                    threshold,
                    signer_vec,
                    expiry_secs,
                    &created_by,
                )
                .await?;
            }
            MultisigCommands::CreateProposal {
                contract_name,
                contract_id,
                wasm_hash,
                network: net_str,
                policy_id,
                proposer,
                description,
            } => {
                log::debug!(
                    "Command: multisig create-proposal | contract_id={} policy_id={}",
                    contract_id,
                    policy_id
                );
                multisig::create_proposal(
                    &cli.api_url,
                    &contract_name,
                    &contract_id,
                    &wasm_hash,
                    &net_str,
                    &policy_id,
                    &proposer,
                    description.as_deref(),
                )
                .await?;
            }
            MultisigCommands::Sign {
                proposal_id,
                signer,
                signature_data,
            } => {
                log::debug!("Command: multisig sign | proposal_id={}", proposal_id);
                multisig::sign_proposal(
                    &cli.api_url,
                    &proposal_id,
                    &signer,
                    signature_data.as_deref(),
                )
                .await?;
            }
            MultisigCommands::Execute { proposal_id } => {
                log::debug!("Command: multisig execute | proposal_id={}", proposal_id);
                multisig::execute_proposal(&cli.api_url, &proposal_id).await?;
            }
            MultisigCommands::Info { proposal_id } => {
                log::debug!("Command: multisig info | proposal_id={}", proposal_id);
                multisig::proposal_info(&cli.api_url, &proposal_id).await?;
            }
            MultisigCommands::ListProposals { status, limit } => {
                log::debug!(
                    "Command: multisig list-proposals | status={:?} limit={}",
                    status,
                    limit
                );
                multisig::list_proposals(&cli.api_url, status.as_deref(), limit).await?;
            }
        },
        Commands::Profile {
            contract_path,
            method,
            output,
            flamegraph,
            compare,
            recommendations,
        } => {
            commands::profile(
                &contract_path,
                method.as_deref(),
                output.as_deref(),
                flamegraph.as_deref(),
                compare.as_deref(),
                recommendations,
            )
            .await?;
        }
        Commands::Test {
            test_file,
            contract_path,
            junit,
            coverage,
            verbose,
        } => {
            commands::run_tests(
                &test_file,
                contract_path.as_deref(),
                junit.as_deref(),
                coverage,
                verbose,
            )
            .await?;
        }
        Commands::Deps { command } => match command {
            DepsCommands::List { contract_id } => {
                commands::deps_list(&cli.api_url, &contract_id).await?;
            }
        },
    }

    Ok(())
}
