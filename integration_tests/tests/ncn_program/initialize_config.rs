#[cfg(test)]
mod tests {
    use ncn_program_core::error::NCNProgramError;
    use solana_program::instruction::InstructionError;
    use solana_sdk::signature::{Keypair, Signer};

    use crate::fixtures::{
        assert_ix_error, ncn_program_client::assert_ncn_program_error, restaking_client::NcnRoot,
        test_builder::TestBuilder, TestResult,
    };

    #[tokio::test]
    async fn test_initialize_ncn_config_ok() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;
        ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_initialize_ncn_config_double_init_fails() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;
        ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;
        fixture.warp_slot_incremental(1).await?;
        let transaction_error = ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await;
        assert_ix_error(transaction_error, InstructionError::InvalidAccountOwner);
        Ok(())
    }

    #[tokio::test]
    async fn test_initialize_ncn_config_invalid_ncn_fails() -> TestResult<()> {
        let fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let fake_ncn = Keypair::new();
        let fake_admin = Keypair::new();
        let fake_ncn_root = NcnRoot {
            ncn_pubkey: fake_ncn.pubkey(),
            ncn_admin: fake_admin,
        };
        ncn_program_client
            .airdrop(&fake_ncn_root.ncn_admin.pubkey(), 1.0)
            .await?;
        let transaction_error = ncn_program_client
            .do_initialize_config(fake_ncn_root.ncn_pubkey, &fake_ncn_root.ncn_admin)
            .await;
        assert_ix_error(transaction_error, InstructionError::InvalidAccountOwner);
        Ok(())
    }

    #[tokio::test]
    async fn test_initialize_ncn_config_invalid_parameters() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;

        // Test invalid epochs_before_stall
        let result = ncn_program_client
            .initialize_config(
                ncn_root.ncn_pubkey,
                &ncn_root.ncn_admin,
                &ncn_root.ncn_admin.pubkey(),
                0, // Invalid - too low
                0,
                10001,
                &ncn_root.ncn_admin.pubkey(), // Use NCN admin as fee wallet
                400,                          // Default fee BPS
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidEpochsBeforeStall, None);

        // Test invalid epochs_before_stall
        let result = ncn_program_client
            .initialize_config(
                ncn_root.ncn_pubkey,
                &ncn_root.ncn_admin,
                &ncn_root.ncn_admin.pubkey(),
                10,
                0, // Invalid - too low
                10001,
                &ncn_root.ncn_admin.pubkey(), // Use NCN admin as fee wallet
                400,                          // Default fee BPS
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidEpochsBeforeClose, None);

        // Test invalid valid_slots_after_consensus
        let result = ncn_program_client
            .initialize_config(
                ncn_root.ncn_pubkey,
                &ncn_root.ncn_admin,
                &ncn_root.ncn_admin.pubkey(),
                5,
                10,
                50,                           // Invalid - too low
                &ncn_root.ncn_admin.pubkey(), // Use NCN admin as fee wallet
                400,                          // Default fee BPS
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidSlotsAfterConsensus, None);

        Ok(())
    }
}
