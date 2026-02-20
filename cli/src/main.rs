mod commands;
mod config;
mod export;
mod import;
mod manifest;

    /// Generate documentation from contract
    Doc {
        /// Path to contract WASM file
        contract_path: String,

        /// Output directory
        #[arg(long, default_value = "docs")]
        output: String,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve network configuration
    let network = config::resolve_network(cli.network)?;

    match cli.command {
        Commands::Search { query, verified_only } => {
            commands::search(&cli.api_url, &query, network, verified_only).await?;
        }
        Commands::Info { contract_id } => {
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
            commands::list(&cli.api_url, limit, network).await?;
        }
        Commands::Migrate {
            contract_id,
            wasm,
            simulate_fail,
            dry_run,
        } => {
            commands::migrate(&cli.api_url, &contract_id, &wasm, simulate_fail, dry_run).await?;
        }
        Commands::Export { id, output, contract_dir } => {
            commands::export(&cli.api_url, &id, &output, &contract_dir).await?;
        }
        Commands::Import { archive, output_dir } => {
            commands::import(&cli.api_url, &archive, network, &output_dir).await?;
        }
        Commands::Doc { contract_path, output } => {
            commands::doc(&contract_path, &output)?;
        }
        Commands::Wizard {} => {
            wizard::run(&cli.api_url).await?;
        }
        Commands::History { search, limit } => {
            wizard::show_history(search.as_deref(), limit)?;
        }
        Commands::Patch { action } => match action {
            PatchCommands::Create { version, hash, severity, rollout } => {
                let sev = severity.parse::<Severity>()?;
                commands::patch_create(&cli.api_url, &version, &hash, sev, rollout).await?;
            }
            PatchCommands::Notify { patch_id } => {
                commands::patch_notify(&cli.api_url, &patch_id).await?;
            }
            PatchCommands::Apply { contract_id, patch_id } => {
                commands::patch_apply(&cli.api_url, &contract_id, &patch_id).await?;
            }
        },
    }
    Ok(())
}
}
