#[cfg(test)]
mod tests {

    use ncn_program_core::constants::WEIGHT;

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[tokio::test]
    async fn test_admin_set_st_mint() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut ncn_program_client = fixture.ncn_program_client();
        let mut vault_client = fixture.vault_client();

        const OPERATOR_COUNT: usize = 1;
        const VAULT_COUNT: usize = 1;

        let test_ncn = fixture
            .create_initial_test_ncn(OPERATOR_COUNT, VAULT_COUNT, None)
            .await?;

        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let vault = vault_client
            .get_vault(&test_ncn.vaults[0].vault_pubkey)
            .await?;
        let st_mint = vault.supported_mint;
        let weight = WEIGHT;

        ncn_program_client
            .do_admin_set_st_mint(ncn, st_mint, weight)
            .await?;

        let vault_registry = ncn_program_client.get_vault_registry(ncn).await?;

        let mint_entry = vault_registry.get_mint_entry(&st_mint).unwrap();

        assert_eq!(*mint_entry.st_mint(), st_mint);
        assert_eq!(mint_entry.weight(), weight);

        ncn_program_client
            .do_admin_set_st_mint(ncn, st_mint, weight)
            .await?;

        let mint_entry = vault_registry.get_mint_entry(&st_mint).unwrap();

        assert_eq!(*mint_entry.st_mint(), st_mint);
        assert_eq!(mint_entry.weight(), weight);

        Ok(())
    }
}
