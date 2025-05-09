use core::fmt;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodU128, PodU64},
    AccountDeserialize, Discriminator,
};
use shank::{ShankAccount, ShankType};
use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::{
    constants::{MAX_ST_MINTS, MAX_VAULTS},
    discriminators::Discriminators,
    error::NCNProgramError,
    loaders::check_load,
};

#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct StMintEntry {
    /// The supported token ( ST ) mint
    st_mint: Pubkey,

    // Either a switchboard feed or a weight must be set
    /// The switchboard feed for the mint
    reserve_switchboard_feed: [u8; 32],
    /// The weight
    weight: PodU128,
}

impl StMintEntry {
    pub fn new(st_mint: &Pubkey, weight: u128) -> Self {
        Self {
            st_mint: *st_mint,
            reserve_switchboard_feed: [0; 32],
            weight: PodU128::from(weight),
        }
    }

    pub fn weight(&self) -> u128 {
        self.weight.into()
    }

    pub const fn st_mint(&self) -> &Pubkey {
        &self.st_mint
    }

    pub fn is_empty(&self) -> bool {
        self.st_mint().eq(&Pubkey::default())
    }
}

impl Default for StMintEntry {
    fn default() -> Self {
        Self::new(&Pubkey::default(), 0)
    }
}

#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct VaultEntry {
    /// The vault account
    vault: Pubkey,
    /// The supported token ( ST ) mint of the vault
    st_mint: Pubkey,
    /// The index of the vault in respect to the NCN account
    vault_index: PodU64,
    /// The slot the vault was registered
    slot_registered: PodU64,
}

impl VaultEntry {
    pub const EMPTY_VAULT_INDEX: u64 = u64::MAX;
    pub const EMPTY_SLOT_REGISTERED: u64 = u64::MAX;

    pub fn new(vault: &Pubkey, st_mint: &Pubkey, vault_index: u64, slot_registered: u64) -> Self {
        Self {
            vault: *vault,
            st_mint: *st_mint,
            vault_index: PodU64::from(vault_index),
            slot_registered: PodU64::from(slot_registered),
        }
    }

    pub const fn vault(&self) -> &Pubkey {
        &self.vault
    }

    pub const fn st_mint(&self) -> &Pubkey {
        &self.st_mint
    }

    pub fn vault_index(&self) -> u64 {
        self.vault_index.into()
    }

    pub fn slot_registered(&self) -> u64 {
        self.slot_registered.into()
    }

    pub fn is_empty(&self) -> bool {
        self.slot_registered() == u64::MAX
    }
}

impl Default for VaultEntry {
    fn default() -> Self {
        Self::new(
            &Pubkey::default(),
            &Pubkey::default(),
            Self::EMPTY_VAULT_INDEX,
            Self::EMPTY_SLOT_REGISTERED,
        )
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod, AccountDeserialize, ShankAccount)]
#[repr(C)]
pub struct VaultRegistry {
    /// The NCN the vault registry is associated with
    pub ncn: Pubkey,
    /// The bump seed for the PDA
    pub bump: u8,
    /// The list of supported token ( ST ) mints
    pub st_mint_list: [StMintEntry; 64],
    /// The list of vaults
    pub vault_list: [VaultEntry; 64],
}

impl Discriminator for VaultRegistry {
    const DISCRIMINATOR: u8 = Discriminators::VaultRegistry as u8;
}

impl VaultRegistry {
    const VAULT_REGISTRY_SEED: &'static [u8] = b"vault_registry";
    pub const SIZE: usize = 8 + size_of::<Self>();

    pub fn new(ncn: &Pubkey, bump: u8) -> Self {
        Self {
            ncn: *ncn,
            bump,
            st_mint_list: [StMintEntry::default(); MAX_ST_MINTS],
            vault_list: [VaultEntry::default(); MAX_VAULTS],
        }
    }

    pub fn initialize(&mut self, ncn: &Pubkey, bump: u8) {
        // Initializes field by field to avoid overflowing stack
        self.ncn = *ncn;
        self.bump = bump;
        self.st_mint_list = [StMintEntry::default(); MAX_ST_MINTS];
        self.vault_list = [VaultEntry::default(); MAX_VAULTS];
    }

    pub fn seeds(ncn: &Pubkey) -> Vec<Vec<u8>> {
        Vec::from_iter(
            [Self::VAULT_REGISTRY_SEED.to_vec(), ncn.to_bytes().to_vec()]
                .iter()
                .cloned(),
        )
    }

    pub fn find_program_address(program_id: &Pubkey, ncn: &Pubkey) -> (Pubkey, u8, Vec<Vec<u8>>) {
        let seeds = Self::seeds(ncn);
        let seeds_iter: Vec<_> = seeds.iter().map(|s| s.as_slice()).collect();
        let (address, bump) = Pubkey::find_program_address(&seeds_iter, program_id);
        (address, bump, seeds)
    }

    pub fn load(
        program_id: &Pubkey,
        account: &AccountInfo,
        ncn: &Pubkey,
        expect_writable: bool,
    ) -> Result<(), ProgramError> {
        let expected_pda = Self::find_program_address(program_id, ncn).0;
        check_load(
            program_id,
            account,
            &expected_pda,
            Some(Self::DISCRIMINATOR),
            expect_writable,
        )
    }

    pub fn has_st_mint(&self, mint: &Pubkey) -> bool {
        self.st_mint_list.iter().any(|m| m.st_mint.eq(mint))
    }

    pub fn check_st_mint_entry(entry: &StMintEntry) -> Result<(), ProgramError> {
        if entry.weight() == 0 {
            return Err(NCNProgramError::WeightNotSet.into());
        }

        Ok(())
    }

    pub fn register_st_mint(&mut self, st_mint: &Pubkey, weight: u128) -> Result<(), ProgramError> {
        // Check if mint is already in the list
        if self.st_mint_list.iter().any(|m| m.st_mint.eq(st_mint)) {
            return Err(NCNProgramError::MintInTable.into());
        }

        // Insert at the first empty slot
        let mint_entry = self
            .st_mint_list
            .iter_mut()
            .find(|m| m.st_mint == StMintEntry::default().st_mint)
            .ok_or(NCNProgramError::VaultRegistryListFull)?;

        let new_mint_entry = StMintEntry::new(st_mint, weight);

        Self::check_st_mint_entry(&new_mint_entry)?;

        *mint_entry = new_mint_entry;

        Ok(())
    }

    pub fn set_st_mint(
        &mut self,
        st_mint: &Pubkey,
        weight: Option<u128>,
    ) -> Result<(), ProgramError> {
        let mint_entry = self
            .st_mint_list
            .iter_mut()
            .find(|m| m.st_mint.eq(st_mint))
            .ok_or(NCNProgramError::MintEntryNotFound)?;

        let mut updated_mint_entry = *mint_entry;

        if let Some(weight) = weight {
            updated_mint_entry.weight = PodU128::from(weight);
        }

        Self::check_st_mint_entry(&updated_mint_entry)?;

        *mint_entry = updated_mint_entry;

        Ok(())
    }

    pub fn register_vault(
        &mut self,
        vault: &Pubkey,
        st_mint: &Pubkey,
        vault_index: u64,
        current_slot: u64,
    ) -> Result<(), ProgramError> {
        // Check if (mint, vault_index) is already in the list
        if self.vault_list.iter().any(|m| m.vault.eq(vault)) {
            return Ok(());
        }

        // Insert at the first empty slot
        let mint_entry = self
            .vault_list
            .iter_mut()
            .find(|m| m.st_mint == VaultEntry::default().st_mint)
            .ok_or(NCNProgramError::VaultRegistryListFull)?;

        *mint_entry = VaultEntry::new(vault, st_mint, vault_index, current_slot);
        Ok(())
    }

    pub const fn get_vault_entries(&self) -> &[VaultEntry; MAX_VAULTS] {
        &self.vault_list
    }

    pub fn vault_count(&self) -> u64 {
        self.vault_list.iter().filter(|m| !m.is_empty()).count() as u64
    }

    pub fn get_valid_vault_entries(&self) -> Vec<VaultEntry> {
        self.vault_list
            .iter()
            .filter(|m| !m.is_empty())
            .copied()
            .collect()
    }

    pub fn get_valid_mint_entries(&self) -> Vec<StMintEntry> {
        self.st_mint_list
            .iter()
            .filter(|m| !m.is_empty())
            .copied()
            .collect()
    }

    pub const fn get_mint_entries(&self) -> &[StMintEntry; MAX_ST_MINTS] {
        &self.st_mint_list
    }

    pub fn st_mint_count(&self) -> usize {
        self.st_mint_list.iter().filter(|m| !m.is_empty()).count()
    }

    pub fn get_mint_entry(&self, st_mint: &Pubkey) -> Result<StMintEntry, ProgramError> {
        let mint_entry = self
            .st_mint_list
            .iter()
            .find(|m| m.st_mint().eq(st_mint))
            .ok_or(NCNProgramError::MintEntryNotFound)?;

        Ok(*mint_entry)
    }
}

#[rustfmt::skip]
impl fmt::Display for VaultRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n\n----------- Vault Registry -------------")?;
        writeln!(f, "  NCN:                          {}", self.ncn)?;
        writeln!(f, "  ST Mints:                     ")?;
        for mint in self.get_valid_mint_entries() {
            writeln!(f, "    Mint:                       {}", mint.st_mint())?;
            writeln!(f, "    Weight:                     {}\n", mint.weight())?;
        }
        writeln!(f, "  Vaults:                     ")?;
        for vault in self.get_valid_vault_entries() {
            writeln!(f, "    Vault:                      {}", vault.vault())?;
            writeln!(f, "      Mint:                     {}", vault.st_mint())?;
            writeln!(f, "      Index:                    {}", vault.vault_index())?;
            writeln!(f, "      Slot Registered:          {}\n", vault.slot_registered())?;
        }


        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::constants::WEIGHT;

    use super::*;

    #[test]
    fn test_len() {
        use std::mem::size_of;

        let expected_total = size_of::<Pubkey>() // ncn
            + 1 // bump
            + size_of::<StMintEntry>() * MAX_ST_MINTS // st_mint_list
            + size_of::<VaultEntry>() * MAX_VAULTS; // vault_list

        assert_eq!(size_of::<VaultRegistry>(), expected_total);

        let vault_registry = VaultRegistry::new(&Pubkey::default(), 0);
        assert_eq!(vault_registry.vault_list.len(), MAX_VAULTS);
    }

    #[test]
    fn test_add_mint() {
        let mut vault_registry = VaultRegistry::new(&Pubkey::default(), 0);
        let mint = Pubkey::new_unique();

        // Test 1: Initial registration should succeed
        assert_eq!(vault_registry.get_valid_mint_entries().len(), 0);
        vault_registry.register_st_mint(&mint, WEIGHT).unwrap();
        assert_eq!(vault_registry.get_valid_mint_entries().len(), 1);

        // Test 2: Trying to add the same mint should fail
        let result = vault_registry.register_st_mint(&mint, WEIGHT);
        assert!(result.is_err());
        assert_eq!(vault_registry.get_valid_mint_entries().len(), 1);

        // Test 3: Adding a different mint should succeed
        let mint2 = Pubkey::new_unique();
        vault_registry.register_st_mint(&mint2, WEIGHT).unwrap();
        assert_eq!(vault_registry.get_valid_mint_entries().len(), 2);

        // Test 4: Verify mint entry data is stored correctly
        let entry = vault_registry.get_mint_entry(&mint).unwrap();
        assert_eq!(entry.st_mint(), &mint);
        assert_eq!(entry.weight(), WEIGHT);

        // Test 5: Adding a mint with weight 0 should fail
        let mint3 = Pubkey::new_unique();
        let result = vault_registry.register_st_mint(&mint3, 0);
        assert!(result.is_err());
        assert_eq!(vault_registry.get_valid_mint_entries().len(), 2);

        // Test 6: Fill up the mint list
        for _ in 2..MAX_ST_MINTS {
            let new_mint = Pubkey::new_unique();
            vault_registry.register_st_mint(&new_mint, WEIGHT).unwrap();
        }

        // Test 7: Attempting to add to a full list should fail
        let overflow_mint = Pubkey::new_unique();
        let result = vault_registry.register_st_mint(&overflow_mint, WEIGHT);
        assert!(result.is_err());
        assert_eq!(vault_registry.get_valid_mint_entries().len(), MAX_ST_MINTS);

        // Test 8: has_st_mint should work correctly
        assert!(vault_registry.has_st_mint(&mint));
        assert!(vault_registry.has_st_mint(&mint2));
        assert!(!vault_registry.has_st_mint(&overflow_mint));

        // Test 9: Test mint with weight instead of switchboard feed
        let mut fresh_registry = VaultRegistry::new(&Pubkey::default(), 0);
        let mint_with_weight = Pubkey::new_unique();
        fresh_registry
            .register_st_mint(&mint_with_weight, WEIGHT)
            .unwrap();

        let entry = fresh_registry.get_mint_entry(&mint_with_weight).unwrap();
        assert_eq!(entry.weight(), 100);
    }

    #[test]
    fn test_set_st_mint() {
        let mut vault_registry = VaultRegistry::new(&Pubkey::default(), 0);
        let mint = Pubkey::new_unique();

        // First register a mint to update
        vault_registry.register_st_mint(&mint, WEIGHT).unwrap();

        // Test 1: Verify initial state
        let entry = vault_registry.get_mint_entry(&mint).unwrap();
        assert_eq!(entry.st_mint(), &mint);
        assert_eq!(entry.weight(), WEIGHT);

        // Test 5: Update weight
        vault_registry.set_st_mint(&mint, Some(100)).unwrap();
        let entry = vault_registry.get_mint_entry(&mint).unwrap();
        assert_eq!(entry.weight(), 100);

        // Test 6: Update multiple fields at once
        vault_registry.set_st_mint(&mint, Some(200)).unwrap();
        let entry = vault_registry.get_mint_entry(&mint).unwrap();
        assert_eq!(entry.weight(), 200);

        // Test 7: Attempt to update non-existent mint
        let nonexistent_mint = Pubkey::new_unique();
        let result = vault_registry.set_st_mint(&nonexistent_mint, None);
        assert_eq!(
            result.unwrap_err(),
            ProgramError::from(NCNProgramError::MintEntryNotFound)
        );

        // Test 8: Setting  weight to invalid values should fail
        let result = vault_registry.set_st_mint(&mint, Some(0));
        assert!(result.is_err());

        // Test 9: Verify original values remain after failed update
        let entry = vault_registry.get_mint_entry(&mint).unwrap();
        assert_eq!(entry.weight(), 200);
    }

    #[test]
    fn test_mint_count() {
        let mut vault_registry = VaultRegistry::new(&Pubkey::default(), 0);
        assert_eq!(vault_registry.vault_count(), 0);

        for i in 0..3 {
            vault_registry
                .register_vault(&Pubkey::new_unique(), &Pubkey::new_unique(), i, 0)
                .unwrap();
        }
        assert_eq!(vault_registry.vault_count(), 3);
    }

    #[test]
    fn test_no_duplicate_mints() {
        let mut vault_registry = VaultRegistry::new(&Pubkey::default(), 0);

        let mint1 = Pubkey::new_unique();
        let mint2 = Pubkey::new_unique();
        vault_registry.register_st_mint(&mint1, WEIGHT).unwrap();
        vault_registry.register_st_mint(&mint2, WEIGHT).unwrap();

        let result = vault_registry.register_st_mint(&mint1, WEIGHT);

        assert!(result.is_err());
    }
}
