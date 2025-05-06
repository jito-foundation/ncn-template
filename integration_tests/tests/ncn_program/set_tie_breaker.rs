#[cfg(test)]
mod tests {

    use ncn_program_core::{
        ballot_box::{Ballot, WeatherStatus},
        constants::DEFAULT_CONSENSUS_REACHED_SLOT,
    };

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_set_tie_breaker() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        // Each operator gets 50% voting share
        let test_ncn = fixture.create_initial_test_ncn(2, 1, None).await?;

        ///// NCNProgram Setup /////
        fixture.snapshot_test_ncn(&test_ncn).await?;

        let clock = fixture.clock().await;
        let epoch = clock.epoch;
        let ncn = test_ncn.ncn_root.ncn_pubkey;

        ncn_program_client
            .do_full_initialize_ballot_box(ncn, epoch)
            .await?;

        let weather_status = WeatherStatus::Sunny as u8;

        let operator = test_ncn.operators[0].operator_pubkey;
        let operator_admin = &test_ncn.operators[0].operator_admin;

        // Cast a vote so that this vote is one of the valid options
        // Gets to 50% consensus weight
        ncn_program_client
            .do_cast_vote(ncn, operator, operator_admin, weather_status, epoch)
            .await?;

        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
        assert!(ballot_box.has_ballot(&Ballot::new(weather_status)));
        assert_eq!(
            ballot_box.slot_consensus_reached(),
            DEFAULT_CONSENSUS_REACHED_SLOT
        );
        assert!(!ballot_box.is_consensus_reached());

        // Wait a bunch of epochs for voting window to expire (TODO use the exact length)
        fixture.warp_slot_incremental(1000000).await?;

        ncn_program_client
            .do_admin_set_tie_breaker(ncn, weather_status, epoch)
            .await?;

        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;

        let ballot = Ballot::new(weather_status);
        assert!(ballot_box.has_ballot(&ballot));
        assert_eq!(
            *ballot_box.get_winning_ballot_tally().unwrap().ballot(),
            ballot
        );
        // No official consensus reached so no slot set
        assert_eq!(
            ballot_box.slot_consensus_reached(),
            DEFAULT_CONSENSUS_REACHED_SLOT
        );
        assert!(ballot_box.is_consensus_reached());

        Ok(())
    }
}
