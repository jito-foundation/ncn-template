mod tests {
    use ncn_program_client::types::ConfigAdminRole;
    use ncn_program_core::{config::Config as NcnConfig, error::NCNProgramError};
    use solana_program::pubkey::Pubkey;
    use solana_sdk::{instruction::InstructionError, signature::Keypair};

    use crate::fixtures::{
        assert_ix_error, ncn_program_client::assert_ncn_program_error, restaking_client::NcnRoot,
        test_builder::TestBuilder, TestResult,
    };

    #[tokio::test]
    async fn test_set_new_admin_success() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;

        ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;

        fixture.warp_slot_incremental(1).await?;

        let new_tie_breaker = Pubkey::new_unique();
        ncn_program_client
            .do_set_new_admin(ConfigAdminRole::TieBreakerAdmin, new_tie_breaker, &ncn_root)
            .await?;

        let config = ncn_program_client
            .get_ncn_config(ncn_root.ncn_pubkey)
            .await?;
        assert_eq!(config.tie_breaker_admin, new_tie_breaker);
        Ok(())
    }

    #[tokio::test]
    async fn test_set_new_admin_incorrect_accounts() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;

        ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;

        fixture.warp_slot_incremental(1).await?;
        let mut restaking_program_client = fixture.restaking_program_client();
        let wrong_ncn_root = restaking_program_client.do_initialize_ncn(None).await?;

        let result = ncn_program_client
            .set_new_admin(
                NcnConfig::find_program_address(&ncn_program::id(), &ncn_root.ncn_pubkey).0,
                ConfigAdminRole::TieBreakerAdmin,
                Pubkey::new_unique(),
                &wrong_ncn_root,
            )
            .await;

        assert_ix_error(result, InstructionError::InvalidAccountData);

        let wrong_ncn_root = NcnRoot {
            ncn_pubkey: ncn_root.ncn_pubkey,
            ncn_admin: Keypair::new(),
        };

        let result = ncn_program_client
            .do_set_new_admin(
                ConfigAdminRole::TieBreakerAdmin,
                Pubkey::new_unique(),
                &wrong_ncn_root,
            )
            .await;

        assert_ncn_program_error(result, NCNProgramError::IncorrectNcnAdmin);
        Ok(())
    }
}
