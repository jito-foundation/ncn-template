#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use ncn_program_core::constants::WEIGHT;
    use solana_sdk::{signature::Keypair, signer::Signer};

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_register_vault_success() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();
        let ncn_root = fixture.setup_ncn().await?;
        // // Setup initial state
        ncn_program_client.setup_ncn_program(&ncn_root).await?;

        // // Setup vault and tickets
        let vault_root = vault_client
            .do_initialize_vault(0, 0, 0, 9, &ncn_root.ncn_pubkey, None)
            .await?;
        restaking_client
            .do_initialize_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
            .await?;
        vault_client
            .do_initialize_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
            .await?;

        let vault = vault_root.vault_pubkey;
        let ncn_vault_ticket = NcnVaultTicket::find_program_address(
            &jito_restaking_program::id(),
            &ncn_root.ncn_pubkey,
            &vault_root.vault_pubkey,
        )
        .0;

        fixture.warp_slot_incremental(2).await?;

        vault_client
            .do_warmup_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
            .await?;
        restaking_client
            .do_warmup_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
            .await?;
        let restaking_config_pubkey = Config::find_program_address(&jito_restaking_program::id()).0;
        let epoch_length = restaking_client
            .get_config(&restaking_config_pubkey)
            .await?
            .epoch_length();
        fixture.warp_slot_incremental(2 * epoch_length).await?;

        let vault_account = vault_client.get_vault(&vault).await?;
        let st_mint = vault_account.supported_mint;

        // Register ST Mint
        ncn_program_client
            .do_admin_register_st_mint(ncn_root.ncn_pubkey, st_mint, WEIGHT)
            .await?;

        // Register mint
        ncn_program_client
            .do_register_vault(ncn_root.ncn_pubkey, vault, ncn_vault_ticket)
            .await?;

        // Verify mint was registered by checking tracked mints
        let vault_registry = ncn_program_client
            .get_vault_registry(ncn_root.ncn_pubkey)
            .await?;
        assert_eq!(vault_registry.vault_count(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_register_vault_fails_without_vault_registry() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;

        // Try to register mint without initialization
        let vault = Keypair::new();
        let ncn_vault_ticket = Keypair::new();

        let result = ncn_program_client
            .do_register_vault(
                ncn_root.ncn_pubkey,
                vault.pubkey(),
                ncn_vault_ticket.pubkey(),
            )
            .await;

        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_register_vault_duplicate() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();
        let ncn_root = fixture.setup_ncn().await?;

        // Setup initial state
        ncn_program_client.setup_ncn_program(&ncn_root).await?;

        // Setup vault and tickets
        let vault_root = vault_client
            .do_initialize_vault(0, 0, 0, 9, &ncn_root.ncn_pubkey, None)
            .await?;
        restaking_client
            .do_initialize_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
            .await?;
        vault_client
            .do_initialize_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
            .await?;

        let vault = vault_root.vault_pubkey;

        let ncn_vault_ticket = NcnVaultTicket::find_program_address(
            &jito_restaking_program::id(),
            &ncn_root.ncn_pubkey,
            &vault_root.vault_pubkey,
        )
        .0;

        fixture.warp_slot_incremental(2).await?;

        vault_client
            .do_warmup_vault_ncn_ticket(&vault_root, &ncn_root.ncn_pubkey)
            .await?;
        restaking_client
            .do_warmup_ncn_vault_ticket(&ncn_root, &vault_root.vault_pubkey)
            .await?;
        let restaking_config_pubkey = Config::find_program_address(&jito_restaking_program::id()).0;
        let epoch_length = restaking_client
            .get_config(&restaking_config_pubkey)
            .await?
            .epoch_length();
        fixture.warp_slot_incremental(2 * epoch_length).await?;

        let vault_account = vault_client.get_vault(&vault).await?;
        let st_mint = vault_account.supported_mint;

        // Register ST Mint
        ncn_program_client
            .do_admin_register_st_mint(ncn_root.ncn_pubkey, st_mint, WEIGHT)
            .await?;

        // Register mint first time
        ncn_program_client
            .do_register_vault(ncn_root.ncn_pubkey, vault, ncn_vault_ticket)
            .await?;

        fixture.warp_slot_incremental(1).await?;

        // Register same mint again
        ncn_program_client
            .do_register_vault(ncn_root.ncn_pubkey, vault, ncn_vault_ticket)
            .await?;

        // Verify mint was only registered once
        let vault_registry = ncn_program_client
            .get_vault_registry(ncn_root.ncn_pubkey)
            .await?;
        assert_eq!(vault_registry.vault_count(), 1);

        Ok(())
    }
}
