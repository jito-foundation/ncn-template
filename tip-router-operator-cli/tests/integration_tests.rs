use std::{fs, path::PathBuf};

use solana_program::stake::state::StakeStateV2;
use solana_program_test::*;
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use tempfile::TempDir;

#[allow(dead_code)]
struct TestContext {
    pub context: ProgramTestContext,
    pub payer: Keypair,
    pub stake_accounts: Vec<Keypair>,
    pub vote_account: Keypair,
    pub temp_dir: TempDir,
    pub output_dir: PathBuf,
}

impl TestContext {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let output_dir = temp_dir.path().join("output");
        fs::create_dir_all(&output_dir)?;

        let program_test = ProgramTest::default();

        let mut context = program_test.start_with_context().await;

        let payer = Keypair::from_bytes(&[
            150, 240, 104, 157, 252, 242, 234, 79, 21, 27, 145, 68, 254, 17, 186, 35, 13, 209, 129,
            229, 55, 39, 221, 2, 10, 15, 172, 77, 153, 153, 104, 177, 139, 35, 180, 131, 48, 220,
            136, 28, 111, 206, 79, 164, 184, 15, 55, 187, 195, 222, 117, 207, 143, 84, 114, 234,
            214, 170, 73, 166, 23, 140, 14, 138,
        ])
        .unwrap();

        let vote_account = Keypair::from_bytes(&[
            82, 63, 68, 226, 112, 24, 184, 190, 189, 221, 199, 191, 113, 6, 183, 211, 49, 118, 207,
            131, 38, 112, 192, 34, 209, 45, 157, 156, 33, 180, 25, 211, 171, 205, 243, 31, 145,
            173, 120, 114, 64, 56, 53, 106, 167, 105, 39, 7, 29, 221, 214, 110, 30, 189, 102, 134,
            182, 90, 143, 73, 233, 179, 44, 215,
        ])
        .unwrap();

        // Fund payer account
        let tx = Transaction::new_signed_with_payer(
            &[system_instruction::transfer(
                &context.payer.pubkey(),
                &payer.pubkey(),
                10_000_000_000, // Increased balance for multiple accounts
            )],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );
        context.banks_client.process_transaction(tx).await?;

        // Create multiple stake accounts
        let stake_accounts = vec![
            Keypair::from_bytes(&[
                36, 145, 249, 6, 56, 206, 144, 159, 252, 235, 120, 107, 227, 51, 95, 155, 16, 93,
                244, 249, 80, 188, 177, 237, 116, 119, 71, 26, 61, 226, 174, 9, 73, 94, 136, 174,
                207, 186, 99, 252, 235, 4, 227, 102, 95, 202, 6, 191, 229, 155, 236, 132, 35, 200,
                218, 165, 164, 223, 77, 9, 74, 55, 87, 167,
            ])
            .unwrap(),
            Keypair::from_bytes(&[
                171, 218, 192, 44, 77, 53, 91, 116, 35, 211, 6, 39, 143, 37, 139, 113, 125, 95, 21,
                51, 238, 233, 23, 186, 6, 224, 117, 203, 24, 130, 12, 102, 184, 8, 146, 226, 205,
                37, 237, 60, 24, 44, 119, 124, 26, 16, 34, 91, 30, 156, 166, 43, 70, 30, 42, 226,
                84, 246, 174, 88, 117, 46, 140, 65,
            ])
            .unwrap(),
            Keypair::from_bytes(&[
                69, 215, 21, 39, 99, 64, 106, 141, 233, 163, 199, 154, 22, 184, 130, 157, 255, 77,
                25, 80, 243, 130, 18, 90, 221, 96, 45, 14, 189, 207, 193, 123, 189, 104, 24, 197,
                242, 185, 90, 22, 166, 44, 253, 177, 199, 207, 211, 235, 146, 157, 84, 203, 205,
                56, 142, 65, 79, 75, 247, 114, 151, 204, 190, 147,
            ])
            .unwrap(),
        ];

        // Get rent and space requirements
        let rent = context.banks_client.get_rent().await?;
        let stake_space = std::mem::size_of::<StakeStateV2>();
        let stake_rent = rent.minimum_balance(stake_space);

        // Initialize each stake account
        for stake_account in stake_accounts.iter() {
            let tx = Transaction::new_signed_with_payer(
                &[
                    system_instruction::create_account(
                        &payer.pubkey(),
                        &stake_account.pubkey(),
                        stake_rent,
                        stake_space as u64,
                        &solana_program::stake::program::id(),
                    ),
                    solana_program::stake::instruction::initialize(
                        &stake_account.pubkey(),
                        &(solana_sdk::stake::state::Authorized {
                            staker: payer.pubkey(),
                            withdrawer: payer.pubkey(),
                        }),
                        &solana_sdk::stake::state::Lockup::default(),
                    ),
                ],
                Some(&payer.pubkey()),
                &[&payer, stake_account],
                context.last_blockhash,
            );
            context.banks_client.process_transaction(tx).await?;

            // Update blockhash between transactions
            context.last_blockhash = context.banks_client.get_latest_blockhash().await?;
        }

        // Create and initialize vote account (if needed)
        // Add vote account initialization here if required

        Ok(Self {
            context,
            payer,
            stake_accounts, // Store all stake accounts instead of just one
            vote_account,
            temp_dir,
            output_dir,
        })
    }
}
