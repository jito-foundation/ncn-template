#![allow(clippy::integer_division)]
use ::{
    anyhow::Result,
    clap::Parser,
    ellipsis_client::EllipsisClient,
    log::{error, info},
    ncn_program_core::ballot_box::WeatherStatus,
    ncn_program_operator_cli::{
        cli::{Cli, Commands},
        load_bank_from_snapshot, process_epoch,
        submit::{submit_recent_epochs_to_ncn, submit_to_ncn},
        Version,
    },
    solana_metrics::{datapoint_info, set_host_id},
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{pubkey::Pubkey, signer::keypair::read_keypair_file},
    std::{str::FromStr, sync::Arc, time::Duration},
    tokio::time::sleep,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    // Ensure backup directory and
    cli.force_different_backup_snapshot_dir();

    let keypair = read_keypair_file(&cli.keypair_path).expect("Failed to read keypair file");
    let rpc_client = EllipsisClient::from_rpc_with_timeout(
        RpcClient::new(cli.rpc_url.clone()),
        &read_keypair_file(&cli.keypair_path).expect("Failed to read keypair file"),
        1_800_000, // 30 minutes
    )?;

    set_host_id(cli.operator_address.to_string());
    datapoint_info!(
        "ncn_program_cli.version",
        ("operator_address", cli.operator_address.to_string(), String),
        ("version", Version::default().to_string(), String)
    );

    info!(
        "CLI Arguments:
        keypair_path: {}
        operator_address: {}
        rpc_url: {}
        ledger_path: {}
        full_snapshots_path: {:?}
        snapshot_output_dir: {}
        backup_snapshots_dir: {}",
        cli.keypair_path,
        cli.operator_address,
        cli.rpc_url,
        cli.ledger_path.display(),
        cli.full_snapshots_path,
        cli.snapshot_output_dir.display(),
        cli.backup_snapshots_dir.display(),
    );

    match cli.command {
        Commands::Run {
            ncn_address,
            ncn_program_id,
            save_snapshot,
            num_monitored_epochs,
            override_target_slot,
            starting_stage,
        } => {
            assert!(
                num_monitored_epochs > 0,
                "num-monitored-epochs must be greater than 0"
            );

            info!("Running NCN Program...");
            info!("NCN Address: {}", ncn_address);
            info!("NCN Program ID: {}", ncn_program_id);
            info!("Save Snapshots: {}", save_snapshot);
            info!("Num Monitored Epochs: {}", num_monitored_epochs);
            info!("Override Target Slot: {:?}", override_target_slot);
            info!("Submit as Memo: {}", cli.submit_as_memo);
            info!("starting stage: {:?}", starting_stage);

            let rpc_client_clone = rpc_client.clone();
            let backup_snapshots_dir = cli.backup_snapshots_dir.clone();
            let cli_clone: Cli = cli.clone();

            if !backup_snapshots_dir.exists() {
                info!(
                    "Creating backup snapshots directory at {}",
                    backup_snapshots_dir.display()
                );
                std::fs::create_dir_all(&backup_snapshots_dir)?;
            }

            // Check for new meta merkle trees and submit to NCN periodically
            tokio::spawn(async move {
                let keypair_arc = Arc::new(keypair);
                loop {
                    if let Err(e) = submit_recent_epochs_to_ncn(
                        &rpc_client_clone,
                        &keypair_arc,
                        &ncn_address,
                        &ncn_program_id,
                        num_monitored_epochs,
                        WeatherStatus::default() as u8,
                        &cli_clone,
                    )
                    .await
                    {
                        error!("Error submitting to NCN: {}", e);
                    }
                    sleep(Duration::from_secs(600)).await;
                }
            });

            // Endless loop that transitions between stages of the operator process.
            process_epoch::loop_stages(
                rpc_client,
                cli,
                starting_stage,
                &ncn_program_id,
                &ncn_address,
                save_snapshot,
            )
            .await?;
        }
        Commands::SnapshotSlot { slot } => {
            info!("Snapshotting slot...");

            load_bank_from_snapshot(cli, slot, true);
        }
        Commands::SubmitEpoch {
            ncn_address,
            ncn_program_id,
            epoch,
        } => {
            let operator_address = Pubkey::from_str(&cli.operator_address)?;
            submit_to_ncn(
                &rpc_client,
                &keypair,
                &operator_address,
                epoch,
                &ncn_address,
                &ncn_program_id,
                WeatherStatus::default() as u8,
                cli.submit_as_memo,
            )
            .await?;
        }
    }
    Ok(())
}
