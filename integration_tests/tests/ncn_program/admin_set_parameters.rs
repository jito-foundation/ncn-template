#[cfg(test)]
mod tests {
    use ncn_program_core::error::NCNProgramError;

    use crate::fixtures::{
        ncn_program_client::assert_ncn_program_error, test_builder::TestBuilder, TestResult,
    };

    #[tokio::test]
    async fn test_admin_set_parameters() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let ncn_root = fixture.setup_ncn().await?;
        ncn_program_client
            .do_initialize_config(ncn_root.ncn_pubkey, &ncn_root.ncn_admin)
            .await?;

        // Test setting valid parameters
        ncn_program_client
            .do_set_parameters(
                None,
                Some(5),    // epochs_before_stall
                Some(10),   // epochs_after_consensus_before_close
                Some(1000), // valid_slots_after_consensus
                &ncn_root,
            )
            .await?;

        // Verify parameters were set
        let config = ncn_program_client
            .get_ncn_config(ncn_root.ncn_pubkey)
            .await?;
        assert_eq!(config.epochs_before_stall(), 5);
        assert_eq!(config.epochs_after_consensus_before_close(), 10);
        assert_eq!(config.valid_slots_after_consensus(), 1000);

        // Test invalid epochs_before_stall
        let result = ncn_program_client
            .do_set_parameters(
                None,
                Some(0), // Invalid - too low
                None,
                None,
                &ncn_root,
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidEpochsBeforeStall, None);

        // Test invalid epochs_before_stall
        let result = ncn_program_client
            .do_set_parameters(
                None,
                None,
                Some(0), // Invalid - too low
                None,
                &ncn_root,
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidEpochsBeforeClose, None);

        // Test invalid valid_slots_after_consensus
        let result = ncn_program_client
            .do_set_parameters(
                None,
                None,
                None,
                Some(99), // Invalid - too low
                &ncn_root,
            )
            .await;
        assert_ncn_program_error(result, NCNProgramError::InvalidSlotsAfterConsensus, None);

        Ok(())
    }
}
