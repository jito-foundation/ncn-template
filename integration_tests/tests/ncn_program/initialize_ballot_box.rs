#[cfg(test)]
mod tests {

    use ncn_program_core::{
        ballot_box::BallotBox,
        constants::{DEFAULT_CONSENSUS_REACHED_SLOT, MAX_REALLOC_BYTES},
    };

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_initialize_ballot_box() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;

        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;

        fixture.warp_slot_incremental(1000).await?;

        let epoch = fixture.clock().await.epoch;

        let ncn = test_ncn.ncn_root.ncn_pubkey;

        let num_reallocs = (ncn_program_core::ballot_box::BallotBox::SIZE as f64
            / ncn_program_core::constants::MAX_REALLOC_BYTES as f64)
            .ceil() as u64
            - 1;

        ncn_program_client
            .do_initialize_ballot_box(ncn, epoch)
            .await?;

        let address = BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch).0;
        let raw_account = fixture.get_account(&address).await?.unwrap();
        assert_eq!(raw_account.data.len(), MAX_REALLOC_BYTES as usize);
        assert_eq!(raw_account.owner, ncn_program::id());
        assert_eq!(raw_account.data[0], 0);

        ncn_program_client
            .do_realloc_ballot_box(ncn, epoch, num_reallocs)
            .await?;

        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;

        assert_eq!(ballot_box.epoch(), epoch);
        assert_eq!(ballot_box.unique_ballots(), 0);
        assert_eq!(ballot_box.operators_voted(), 0);
        assert!(!ballot_box.is_consensus_reached());
        assert_eq!(
            ballot_box.slot_consensus_reached(),
            DEFAULT_CONSENSUS_REACHED_SLOT
        );
        assert!(ballot_box.get_winning_ballot_tally().is_err(),);

        Ok(())
    }
}
