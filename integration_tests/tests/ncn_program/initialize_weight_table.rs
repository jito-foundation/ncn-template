#[cfg(test)]
mod tests {
    use jito_bytemuck::Discriminator;
    use ncn_program_core::{
        constants::{MAX_REALLOC_BYTES, MAX_VAULTS},
        weight_table::WeightTable,
    };

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_initialize_weight_table_ok() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        fixture.warp_slot_incremental(1000).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        let ncn = test_ncn.ncn_root.ncn_pubkey;

        ncn_program_client
            .do_initialize_weight_table(ncn, epoch)
            .await?;

        let address = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.data.len(), MAX_REALLOC_BYTES as usize);
        assert_eq!(raw_account.owner, ncn_program::id());
        assert_eq!(raw_account.data[0], 0);

        let num_reallocs = (WeightTable::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;

        ncn_program_client
            .do_realloc_weight_table(ncn, epoch, num_reallocs)
            .await?;

        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.data.len(), { WeightTable::SIZE });
        assert_eq!(raw_account.owner, ncn_program::id());
        assert_eq!(raw_account.data[0], WeightTable::DISCRIMINATOR);

        let weight_table = ncn_program_client.get_weight_table(ncn, epoch).await?;

        assert_eq!(*weight_table.ncn(), ncn);
        assert_eq!(weight_table.ncn_epoch(), epoch);

        Ok(())
    }

    #[tokio::test]
    async fn test_initialize_max_weight_table_ok() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, MAX_VAULTS, None).await?;
        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        fixture.warp_slot_incremental(1000).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        let ncn = test_ncn.ncn_root.ncn_pubkey;

        ncn_program_client
            .do_initialize_weight_table(ncn, epoch)
            .await?;

        let address = WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.data.len(), MAX_REALLOC_BYTES as usize);
        assert_eq!(raw_account.owner, ncn_program::id());
        assert_eq!(raw_account.data[0], 0);

        let num_reallocs = (WeightTable::SIZE as f64 / MAX_REALLOC_BYTES as f64).ceil() as u64 - 1;

        ncn_program_client
            .do_realloc_weight_table(ncn, epoch, num_reallocs)
            .await?;

        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.data.len(), { WeightTable::SIZE });
        assert_eq!(raw_account.owner, ncn_program::id());
        assert_eq!(raw_account.data[0], WeightTable::DISCRIMINATOR);

        let weight_table = ncn_program_client.get_weight_table(ncn, epoch).await?;

        assert_eq!(*weight_table.ncn(), ncn);
        assert_eq!(weight_table.ncn_epoch(), epoch);

        Ok(())
    }
}
