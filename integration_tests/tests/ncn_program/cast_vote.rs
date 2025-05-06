#[cfg(test)]
mod tests {
    use ncn_program_core::{
        ballot_box::{Ballot, WeatherStatus},
        constants::MAX_OPERATORS,
        error::NCNProgramError,
    };
    use rand::Rng;
    use solana_sdk::msg;

    use crate::fixtures::{
        ncn_program_client::assert_ncn_program_error, test_builder::TestBuilder, TestResult,
    };

    #[tokio::test]
    async fn test_cast_vote() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(1, 1, None).await?;

        ///// NCNProgram Setup /////
        fixture.warp_slot_incremental(1000).await?;

        fixture.snapshot_test_ncn(&test_ncn).await?;
        //////

        let clock = fixture.clock().await;
        let slot = clock.slot;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let operator = test_ncn.operators[0].operator_pubkey;
        let epoch = clock.epoch;

        ncn_program_client
            .do_full_initialize_ballot_box(ncn, epoch)
            .await?;

        let weather_status = Ballot::generate_ballot_weather_status();

        let operator_admin = &test_ncn.operators[0].operator_admin;

        ncn_program_client
            .do_cast_vote(ncn, operator, operator_admin, weather_status, epoch)
            .await?;

        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;

        assert!(ballot_box.has_ballot(&Ballot::new(weather_status)));
        assert_eq!(ballot_box.slot_consensus_reached(), slot);
        assert!(ballot_box.is_consensus_reached());

        Ok(())
    }

    #[tokio::test]
    async fn test_operator_cannot_vote_twice() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(3, 1, None).await?;

        ///// NCNProgram Setup /////
        fixture.warp_slot_incremental(1000).await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        //////

        let clock = fixture.clock().await;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let operator = test_ncn.operators[0].operator_pubkey;
        let operator_admin = &test_ncn.operators[0].operator_admin;
        let epoch = clock.epoch;

        // Initialize ballot box
        ncn_program_client
            .do_full_initialize_ballot_box(ncn, epoch)
            .await?;

        // First vote should succeed
        let first_weather_status = WeatherStatus::Sunny as u8;
        ncn_program_client
            .do_cast_vote(ncn, operator, operator_admin, first_weather_status, epoch)
            .await?;

        // Verify first vote was recorded
        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
        assert!(ballot_box.has_ballot(&Ballot::new(first_weather_status)));
        assert_eq!(ballot_box.operators_voted(), 1);
        assert_eq!(ballot_box.unique_ballots(), 1);

        // Second vote should fail
        let second_weather_status = WeatherStatus::Cloudy as u8;
        let result = ncn_program_client
            .do_cast_vote(ncn, operator, operator_admin, second_weather_status, epoch)
            .await;

        msg!("result: {:?}", result);
        assert_ncn_program_error(result, NCNProgramError::OperatorAlreadyVoted);

        // Verify ballot box state remains unchanged
        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
        assert!(ballot_box.has_ballot(&Ballot::new(first_weather_status)));
        assert!(!ballot_box.has_ballot(&Ballot::new(second_weather_status)));
        assert_eq!(ballot_box.operators_voted(), 1);
        assert_eq!(ballot_box.unique_ballots(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_bad_ballot() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture.create_initial_test_ncn(3, 1, None).await?;

        ///// NCNProgram Setup /////
        fixture.warp_slot_incremental(1000).await?;

        fixture.snapshot_test_ncn(&test_ncn).await?;
        //////

        let clock = fixture.clock().await;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let operator = test_ncn.operators[0].operator_pubkey;
        let epoch = clock.epoch;

        ncn_program_client
            .do_full_initialize_ballot_box(ncn, epoch)
            .await?;

        let weather_status = 5;

        let operator_admin = &test_ncn.operators[0].operator_admin;

        let result = ncn_program_client
            .do_cast_vote(ncn, operator, operator_admin, weather_status, epoch)
            .await;

        assert_ncn_program_error(result, NCNProgramError::BadBallot);

        Ok(())
    }

    #[ignore = "long test"]
    #[tokio::test]
    async fn test_cast_vote_max_cu() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        let test_ncn = fixture
            .create_initial_test_ncn(MAX_OPERATORS, 1, None)
            .await?;

        ///// NCNProgram Setup /////
        fixture.warp_slot_incremental(1000).await?;

        fixture.snapshot_test_ncn(&test_ncn).await?;
        //////

        let clock = fixture.clock().await;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch = clock.epoch;

        ncn_program_client
            .do_full_initialize_ballot_box(ncn, epoch)
            .await?;

        for operator in test_ncn.operators {
            let operator_admin = &operator.operator_admin;

            let weather_status = rand::rng().random_range(0..=2);

            ncn_program_client
                .do_cast_vote(
                    ncn,
                    operator.operator_pubkey,
                    operator_admin,
                    weather_status,
                    epoch,
                )
                .await?;

            let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
            assert!(ballot_box.has_ballot(&Ballot::new(weather_status)));
        }

        let ballot_box = ncn_program_client.get_ballot_box(ncn, epoch).await?;
        msg!("ballot_box: {}", ballot_box);
        assert!(!ballot_box.is_consensus_reached());

        Ok(())
    }
}
