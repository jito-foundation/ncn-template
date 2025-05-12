#[cfg(test)]
mod tests {

    use ncn_program_core::{epoch_snapshot::OperatorSnapshot, error::NCNProgramError};

    use crate::fixtures::{
        ncn_program_client::assert_ncn_program_error, test_builder::TestBuilder, TestResult,
    };

    #[tokio::test]
    async fn test_initialize_operator_snapshot() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        fixture.warp_slot_incremental(1000).await?;

        fixture.add_weights_for_test_ncn(&test_ncn).await?;
        fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let operator = test_ncn.operators[0].operator_pubkey;

        // Initialize operator snapshot
        ncn_program_client
            .do_initialize_operator_snapshot(operator, ncn, epoch)
            .await?;

        // Check initial size is MAX_REALLOC_BYTES
        let address =
            OperatorSnapshot::find_program_address(&ncn_program::id(), &operator, &ncn, epoch).0;
        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.owner, ncn_program::id());

        // Get operator snapshot and verify it was initialized correctly
        let operator_snapshot = ncn_program_client
            .get_operator_snapshot(operator, ncn, epoch)
            .await?;

        // Verify initial state
        assert_eq!(*operator_snapshot.operator(), operator);
        assert_eq!(*operator_snapshot.ncn(), ncn);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_operator_after_epoch_snapshot() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let mut test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        fixture.warp_slot_incremental(1000).await?;

        fixture.add_weights_for_test_ncn(&test_ncn).await?;
        fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;

        // Add New Operator
        fixture
            .add_operators_to_test_ncn(&mut test_ncn, 1, None)
            .await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        // Last added operator
        let operator = test_ncn.operators[1].operator_pubkey;

        // Initialize operator snapshot
        let result = ncn_program_client
            .do_initialize_operator_snapshot(operator, ncn, epoch)
            .await;

        assert_ncn_program_error(result, NCNProgramError::OperatorIsNotInSnapshot, None);

        Ok(())
    }
}
