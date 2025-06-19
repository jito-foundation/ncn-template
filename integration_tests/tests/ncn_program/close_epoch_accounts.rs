#[cfg(test)]
mod tests {

    use ncn_program_core::ballot_box::BallotBox;
    use ncn_program_core::epoch_snapshot::{EpochSnapshot, OperatorSnapshot};
    use ncn_program_core::ncn_reward_router::{NCNRewardReceiver, NCNRewardRouter};
    use ncn_program_core::operator_vault_reward_router::{
        OperatorVaultRewardReceiver, OperatorVaultRewardRouter,
    };
    use ncn_program_core::weight_table::WeightTable;
    use ncn_program_core::{epoch_state::EpochState, error::NCNProgramError};
    use solana_sdk::msg;

    use crate::fixtures::TestResult;
    use crate::fixtures::{
        ncn_program_client::assert_ncn_program_error, test_builder::TestBuilder,
    };

    #[tokio::test]
    async fn close_all_epoch_accounts_ok() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;

        const OPERATOR_COUNT: usize = 3;
        const VAULT_COUNT: usize = 2;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;
        fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

        Ok(())
    }

    #[tokio::test]
    async fn cannot_close_before_enough_epochs_after_consensus() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch_to_close = fixture.clock().await.epoch;

        // Try Close Epoch State
        {
            let (epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, epoch_state)
                .await;

            assert_ncn_program_error(
                result,
                NCNProgramError::CannotCloseAccountNotEnoughEpochs,
                None,
            );

            let result = fixture.get_account(&epoch_state).await?;
            assert!(result.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn cannot_close_before_consensus_is_reached() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch_to_close = fixture.clock().await.epoch;

        // Warp to way after close
        {
            let config: ncn_program_core::config::Config =
                fixture.ncn_program_client().get_ncn_config(ncn).await?;
            let epochs_after_consensus_before_close = config.epochs_after_consensus_before_close();

            fixture
                .warp_epoch_incremental(epochs_after_consensus_before_close * 2)
                .await?;
        }

        // Try Close Epoch State
        {
            let (epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, epoch_state)
                .await;

            assert_ncn_program_error(result, NCNProgramError::ConsensusNotReached, None);

            let result = fixture.get_account(&epoch_state).await?;
            assert!(result.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn cannot_close_epoch_state_before_others() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch_to_close = fixture.clock().await.epoch;

        // Warp to epoch to close
        {
            let config: ncn_program_core::config::Config =
                fixture.ncn_program_client().get_ncn_config(ncn).await?;
            let epochs_after_consensus_before_close = config.epochs_after_consensus_before_close();

            fixture
                .warp_epoch_incremental(epochs_after_consensus_before_close + 1)
                .await?;
        }

        // Try Close Epoch State
        {
            let (epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, epoch_state)
                .await;

            assert_ncn_program_error(result, NCNProgramError::CannotCloseEpochStateAccount, None);

            let result = fixture.get_account(&epoch_state).await?;
            assert!(result.is_some());
        }

        Ok(())
    }

    #[tokio::test]
    async fn cannot_close_closed_account() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch_to_close = fixture.clock().await.epoch;

        // Warp to epoch to close
        {
            let config: ncn_program_core::config::Config =
                fixture.ncn_program_client().get_ncn_config(ncn).await?;
            let epochs_after_consensus_before_close = config.epochs_after_consensus_before_close();

            fixture
                .warp_epoch_incremental(epochs_after_consensus_before_close + 1)
                .await?;
        }

        // Close Weight Table
        {
            let (weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, weight_table)
                .await?;

            let result = fixture.get_account(&weight_table).await?;
            assert!(result.is_none());
        }

        // Try Close Weight Table Again
        {
            fixture.warp_epoch_incremental(1).await?;

            let (weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, weight_table)
                .await;

            assert_ncn_program_error(
                result,
                NCNProgramError::CannotCloseAccountAlreadyClosed,
                None,
            );

            let result = fixture.get_account(&weight_table).await?;
            assert!(result.is_none());
        }

        Ok(())
    }

    #[tokio::test]
    async fn cannot_reopen_accounts() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let epoch_to_close = fixture.clock().await.epoch;

        // Warp to epoch to close
        {
            let config: ncn_program_core::config::Config =
                fixture.ncn_program_client().get_ncn_config(ncn).await?;
            let epochs_after_consensus_before_close = config.epochs_after_consensus_before_close();

            fixture
                .warp_epoch_incremental(epochs_after_consensus_before_close + 1)
                .await?;
        }

        // Close Weight Table
        {
            let (weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, weight_table)
                .await?;

            let result = fixture.get_account(&weight_table).await?;
            assert!(result.is_none());
        }
        // Try To Create Weight table again
        {
            let (weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_initialize_weight_table(ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::EpochIsClosingDown, None);

            let result = fixture.get_account(&weight_table).await?;
            assert!(result.is_none());
        }

        // Close Epoch Snapshot
        {
            let (epoch_snapshot, _, _) =
                EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, epoch_snapshot)
                .await?;

            let result = fixture.get_account(&epoch_snapshot).await?;
            assert!(result.is_none());
        }
        // Try To Create Epoch Snapshot again
        {
            let (epoch_snapshot, _, _) =
                EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_initialize_epoch_snapshot(ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::EpochIsClosingDown, None);

            let result = fixture.get_account(&epoch_snapshot).await?;
            assert!(result.is_none());
        }

        // Close Operator Snapshot
        {
            let operator = test_ncn.operators[0].operator_pubkey;
            let (operator_snapshot, _, _) = OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close,
            );

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, operator_snapshot)
                .await?;

            let result = fixture.get_account(&operator_snapshot).await?;
            assert!(result.is_none());
        }
        // Try To Create Operator Snapshot again
        {
            let operator = test_ncn.operators[0].operator_pubkey;
            let (operator_snapshot, _, _) = OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close,
            );

            let result = ncn_program_client
                .do_initialize_operator_snapshot(operator, ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::EpochIsClosingDown, None);

            let result = fixture.get_account(&operator_snapshot).await?;
            assert!(result.is_none());
        }

        // Close Ballot Box
        {
            let (ballot_box, _, _) =
                BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, ballot_box)
                .await?;

            let result = fixture.get_account(&ballot_box).await?;
            assert!(result.is_none());
        }
        // Try To Create Ballot Box again
        {
            let (ballot_box, _, _) =
                BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_initialize_ballot_box(ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::EpochIsClosingDown, Some(1));

            let result = fixture.get_account(&ballot_box).await?;
            assert!(result.is_none());
        }

        // Close NCN Reward Router
        {
            let (ncn_reward_router, _, _) =
                NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let (receiver, _, _) =
                NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_router_epoch_account(ncn, epoch_to_close, ncn_reward_router, receiver)
                .await?;
        }

        // try to create NCN Reward Router again
        {
            let (ncn_reward_router, _, _) =
                NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_initialize_ncn_reward_router(ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::EpochIsClosingDown, None);

            let result = fixture.get_account(&ncn_reward_router).await?;
            assert!(result.is_none());
        }

        // Close Operator Vault Reward Router
        for operator_root in test_ncn.operators.iter() {
            let operator = operator_root.operator_pubkey;
            let (operator_vault_reward_router, _, _) =
                OperatorVaultRewardRouter::find_program_address(
                    &ncn_program::id(),
                    &operator,
                    &ncn,
                    epoch_to_close,
                );

            let (operator_vault_reward_receiver, _, _) =
                OperatorVaultRewardReceiver::find_program_address(
                    &ncn_program::id(),
                    &operator,
                    &ncn,
                    epoch_to_close,
                );

            ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    operator_vault_reward_router,
                    operator_vault_reward_receiver,
                )
                .await?;
        }

        // Close Epoch State
        {
            let (epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, epoch_state)
                .await?;

            let result = fixture.get_account(&epoch_state).await?;
            assert!(result.is_none());
        }
        // Try To Create Epoch State again
        {
            let (epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let result = ncn_program_client
                .do_intialize_epoch_state(ncn, epoch_to_close)
                .await;

            assert_ncn_program_error(result, NCNProgramError::MarkerExists, None);

            let result = fixture.get_account(&epoch_state).await?;
            assert!(result.is_none());
        }

        Ok(())
    }

    #[tokio::test]
    async fn cannot_close_wrong_epoch_or_ncn_accounts() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;
        fixture.snapshot_test_ncn(&test_ncn).await?;
        fixture.vote_test_ncn(&test_ncn).await?;
        fixture.reward_test_ncn(&test_ncn, 10_000).await?;

        let epoch_to_close = fixture.clock().await.epoch;

        let mut bad_test_ncn = fixture.create_test_ncn().await?;

        ncn_program_client
            .setup_ncn_program(&bad_test_ncn.ncn_root)
            .await?;

        fixture
            .add_operators_to_test_ncn(&mut bad_test_ncn, OPERATOR_COUNT, None)
            .await?;
        fixture
            .add_vaults_to_test_ncn(&mut bad_test_ncn, VAULT_COUNT, None)
            .await?;
        fixture
            .add_delegation_in_test_ncn(&bad_test_ncn, 100)
            .await?;
        fixture
            .add_vault_registry_to_test_ncn(&bad_test_ncn)
            .await?;
        fixture.snapshot_test_ncn(&bad_test_ncn).await?;
        fixture.vote_test_ncn(&bad_test_ncn).await?;
        fixture.reward_test_ncn(&bad_test_ncn, 10_000).await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let bad_ncn = bad_test_ncn.ncn_root.ncn_pubkey;

        // Warp to epoch to close
        {
            let config: ncn_program_core::config::Config =
                fixture.ncn_program_client().get_ncn_config(ncn).await?;
            let epochs_after_consensus_before_close = config.epochs_after_consensus_before_close();

            fixture
                .warp_epoch_incremental(epochs_after_consensus_before_close + 1)
                .await?;
        }

        // Try Close Bad Weight Table
        {
            let (bad_epoch_weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close + 1);
            let (bad_ncn_weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &bad_ncn, epoch_to_close);
            let (good_weight_table, _, _) =
                WeightTable::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let bad_epoch_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_epoch_weight_table)
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_ncn_weight_table)
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, good_weight_table)
                .await?;
        }

        // Try Close Bad Epoch Snapshot
        {
            let (bad_epoch_epoch_snapshot, _, _) =
                EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch_to_close + 1);
            let (bad_ncn_epoch_snapshot, _, _) =
                EpochSnapshot::find_program_address(&ncn_program::id(), &bad_ncn, epoch_to_close);
            let (good_epoch_snapshot, _, _) =
                EpochSnapshot::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let bad_epoch_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_epoch_epoch_snapshot)
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_ncn_epoch_snapshot)
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, good_epoch_snapshot)
                .await?;
        }

        // Try Close Bad Operator Snapshot
        {
            let operator = test_ncn.operators[0].operator_pubkey;
            let (bad_epoch_operator_snapshot, _, _) = OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close + 1,
            );
            let (bad_ncn_operator_snapshot, _, _) = OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &bad_ncn,
                epoch_to_close,
            );
            let (good_operator_snapshot, _, _) = OperatorSnapshot::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close,
            );

            let bad_epoch_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_epoch_operator_snapshot)
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_ncn_operator_snapshot)
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, good_operator_snapshot)
                .await?;
        }

        // Try Close Bad Ballot Box
        {
            let (bad_epoch_ballot_box, _, _) =
                BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch_to_close + 1);
            let (bad_ncn_ballot_box, _, _) =
                BallotBox::find_program_address(&ncn_program::id(), &bad_ncn, epoch_to_close);
            let (good_ballot_box, _, _) =
                BallotBox::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let bad_epoch_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_epoch_ballot_box)
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_ncn_ballot_box)
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, good_ballot_box)
                .await?;
        }

        // Try Close Bad Base Reward Router
        {
            let (bad_epoch_base_reward_router, _, _) =
                NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch_to_close + 1);
            let (bad_ncn_base_reward_router, _, _) =
                NCNRewardRouter::find_program_address(&ncn_program::id(), &bad_ncn, epoch_to_close);
            let (good_base_reward_router, _, _) =
                NCNRewardRouter::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let (receiver, _, _) =
                NCNRewardReceiver::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let bad_epoch_result = ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    bad_epoch_base_reward_router,
                    receiver,
                )
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    bad_ncn_base_reward_router,
                    receiver,
                )
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    good_base_reward_router,
                    receiver,
                )
                .await?;
        }

        // Try Close Bad NCN Reward Router
        {
            let operator = test_ncn.operators[0].operator_pubkey;
            let (bad_epoch_ncn_reward_router, _, _) =
                OperatorVaultRewardRouter::find_program_address(
                    &ncn_program::id(),
                    &operator,
                    &ncn,
                    epoch_to_close + 1,
                );
            let (bad_ncn_ncn_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
                &ncn_program::id(),
                &operator,
                &bad_ncn,
                epoch_to_close,
            );
            let (good_ncn_reward_router, _, _) = OperatorVaultRewardRouter::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close,
            );

            let (receiver, _, _) = OperatorVaultRewardReceiver::find_program_address(
                &ncn_program::id(),
                &operator,
                &ncn,
                epoch_to_close,
            );

            let bad_epoch_result = ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    bad_epoch_ncn_reward_router,
                    receiver,
                )
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    bad_ncn_ncn_reward_router,
                    receiver,
                )
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_router_epoch_account(
                    ncn,
                    epoch_to_close,
                    good_ncn_reward_router,
                    receiver,
                )
                .await?;
        }

        // Try Close Bad Epoch State
        {
            let (bad_epoch_epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close + 1);
            let (bad_ncn_epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &bad_ncn, epoch_to_close);
            let (good_epoch_state, _, _) =
                EpochState::find_program_address(&ncn_program::id(), &ncn, epoch_to_close);

            let bad_epoch_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_epoch_epoch_state)
                .await;

            let bad_ncn_result = ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, bad_ncn_epoch_state)
                .await;

            assert!(bad_epoch_result.is_err());
            assert!(bad_ncn_result.is_err());

            ncn_program_client
                .do_close_epoch_account(ncn, epoch_to_close, good_epoch_state)
                .await?;
        }

        Ok(())
    }
}
