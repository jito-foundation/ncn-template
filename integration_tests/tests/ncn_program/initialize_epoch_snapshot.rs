#[cfg(test)]
mod tests {

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_initialize_epoch_snapshot_ok() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
        fixture.add_weights_for_test_ncn(&test_ncn).await?;

        let epoch = fixture.clock().await.epoch;

        ncn_program_client
            .do_initialize_epoch_snapshot(test_ncn.ncn_root.ncn_pubkey, epoch)
            .await?;

        Ok(())
    }
}
