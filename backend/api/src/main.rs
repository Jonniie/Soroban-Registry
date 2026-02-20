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

use anyhow::Result;
use clap::{Parser, Subcommand};
use patch::Severity;

/// Soroban Registry CLI — discover, publish, verify, and deploy Soroban contracts
#[derive(Debug, Parser)]
#[command(name = "soroban-registry", version, about, long_about = None)]
pub struct Cli {
    /// Registry API URL
    #[arg(long, env = "SOROBAN_REGISTRY_API_URL")]
    pub api_url: Option<String>,

    /// Stellar network to use (mainnet | testnet | futurenet)
    #[arg(long, global = true)]
    pub network: Option<String>,

    /// HTTP timeout in seconds
    #[arg(long, global = true)]
    pub timeout: Option<u64>,

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
        /// Search query
        query: String,
        /// Only show verified contracts
        #[arg(long)]
        verified_only: bool,
    },

    /// Get detailed information about a contract
    Info {
        /// Contract ID to look up
        contract_id: String,
    },

@@ -134,50 +134,57 @@ pub enum Commands {

    /// Generate documentation from a contract WASM
    Doc {
        /// Path to contract WASM file
        contract_path: String,

        /// Output directory
        #[arg(long, default_value = "docs")]
        output: String,
    },

    /// Launch the interactive setup wizard
    Wizard {},

    /// Show command history
    History {
        /// Filter by search term
        #[arg(long)]
        search: Option<String>,

        /// Maximum number of entries to show
        #[arg(long, default_value = "20")]
        limit: usize,
    },

    /// Manage CLI configuration
    Config {
        /// Open config file in your editor
        #[arg(long)]
        edit: bool,
    },

    /// Security patch management
    Patch {
        #[command(subcommand)]
        action: PatchCommands,
    },

    /// Multi-signature contract deployment workflow
    Multisig {
        #[command(subcommand)]
        action: MultisigCommands,
    },

    /// Profile contract execution performance
    Profile {
        /// Path to contract file
        contract_path: String,

        /// Method to profile
        #[arg(long)]
        method: Option<String>,

        /// Output JSON file
        #[arg(long)]
        output: Option<String>,

@@ -304,195 +311,312 @@ pub enum PatchCommands {
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
    let runtime_config = config::resolve_runtime_config(cli.network, cli.api_url, cli.timeout)?;
    log::debug!("API URL: {}", runtime_config.api_base);

    // ── Resolve network ───────────────────────────────────────────────────────
    let network = runtime_config.network;
    log::debug!("Network: {:?}", network);
    log::debug!("Timeout: {}s", runtime_config.timeout);

    match cli.command {
        Commands::Search {
            query,
            verified_only,
        } => {
            log::debug!(
                "Command: search | query={:?} verified_only={}",
                query,
                verified_only
            );
            commands::search(&runtime_config.api_base, &query, network, verified_only).await?;
        }
        Commands::Info { contract_id } => {
            log::debug!("Command: info | contract_id={}", contract_id);
            commands::info(&runtime_config.api_base, &contract_id, network).await?;
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
                &runtime_config.api_base,
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
            commands::list(&runtime_config.api_base, limit, network).await?;
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
            commands::migrate(
                &runtime_config.api_base,
                &contract_id,
                &wasm,
                simulate_fail,
                dry_run,
            )
            .await?;
        }
        Commands::Export {
            id,
            output,
            contract_dir,
        } => {
            log::debug!("Command: export | id={} output={}", id, output);
            commands::export(&runtime_config.api_base, &id, &output, &contract_dir).await?;
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
            commands::import(&runtime_config.api_base, &archive, network, &output_dir).await?;
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
            wizard::run(&runtime_config.api_base).await?;
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
                commands::patch_create(&runtime_config.api_base, &version, &hash, sev, rollout)
                    .await?;
            }
            PatchCommands::Notify { patch_id } => {
                log::debug!("Command: patch notify | patch_id={}", patch_id);
                commands::patch_notify(&runtime_config.api_base, &patch_id).await?;
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
                commands::patch_apply(&runtime_config.api_base, &contract_id, &patch_id).await?;
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
                    &runtime_config.api_base,
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
                    &runtime_config.api_base,
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
                    &runtime_config.api_base,
                    &proposal_id,
                    &signer,
                    signature_data.as_deref(),
                )
                .await?;
            }
            MultisigCommands::Execute { proposal_id } => {
                log::debug!("Command: multisig execute | proposal_id={}", proposal_id);
                multisig::execute_proposal(&runtime_config.api_base, &proposal_id).await?;
            }
            MultisigCommands::Info { proposal_id } => {
                log::debug!("Command: multisig info | proposal_id={}", proposal_id);
                multisig::proposal_info(&runtime_config.api_base, &proposal_id).await?;
            }
            MultisigCommands::ListProposals { status, limit } => {
                log::debug!(
                    "Command: multisig list-proposals | status={:?} limit={}",
                    status,
                    limit
                );
                multisig::list_proposals(&runtime_config.api_base, status.as_deref(), limit)
                    .await?;
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
                commands::deps_list(&runtime_config.api_base, &contract_id).await?;
            }
        },
        Commands::Config { edit } => {
            if edit {
                config::edit_config()?;
            } else {
                config::show_config()?;
            }
        }
    }

    Ok(())
}
