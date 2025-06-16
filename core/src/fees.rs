use bytemuck::{Pod, Zeroable};
use jito_bytemuck::types::{PodU16, PodU64};
use shank::ShankType;
use solana_program::pubkey::Pubkey;
use spl_math::precise_number::PreciseNumber;

use crate::{constants::MAX_FEE_BPS, error::NCNProgramError};

/// Fee Configuration with Epoch-Delayed Updates
///
/// This system allows for fee updates to take place in a future epoch without requiring
/// immediate updates during the current epoch. This is critical so all operators calculate
/// the same Merkle root regardless of when fee changes are proposed during an epoch.
///
/// The dual fee structure (fee_1 and fee_2) allows one fee to be active while the other
/// is being prepared for a future epoch. On epoch boundaries, the system switches to
/// the fee with the higher activation epoch.
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct FeeConfig {
    /// Jito DAO wallet that receives DAO fees
    jito_dao_fee_wallet: Pubkey,
    /// Jito DAO fee in basis points (1/100 of 1%)
    jito_dao_fee_bps: PodU16,

    /// NCN wallet that receives NCN fees
    ncn_fee_wallets: Pubkey,
    /// NCN fee in basis points (1/100 of 1%)
    ncn_fee_bps: PodU16,

    /// Primary fee configuration (used for active or future epoch)
    fee_1: Fees,
    /// Secondary fee configuration (used for active or future epoch)
    fee_2: Fees,
}

impl FeeConfig {
    /// Creates a new fee configuration with initial values
    /// All fee amounts are validated to ensure they don't exceed maximum allowed values
    pub fn new(
        jito_dao_fee_wallet: &Pubkey,
        jito_dao_fee_bps: u16,
        ncn_fee_wallet: &Pubkey,
        default_ncn_fee_bps: u16,
        current_epoch: u64,
    ) -> Result<Self, NCNProgramError> {
        if jito_dao_fee_wallet.eq(&Pubkey::default()) {
            return Err(NCNProgramError::DefaultDaoWallet);
        }

        if jito_dao_fee_bps as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        if ncn_fee_wallet.eq(&Pubkey::default()) {
            return Err(NCNProgramError::DefaultNcnWallet);
        }

        if default_ncn_fee_bps as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        let fee = Fees::new(jito_dao_fee_bps, default_ncn_fee_bps, current_epoch)?;

        let mut fee_config = Self {
            jito_dao_fee_wallet: *jito_dao_fee_wallet,
            jito_dao_fee_bps: jito_dao_fee_bps.into(),
            ncn_fee_bps: default_ncn_fee_bps.into(),
            ncn_fee_wallets: *ncn_fee_wallet,

            fee_1: fee,
            fee_2: fee,
        };

        fee_config.set_jito_dao_fee_wallet(jito_dao_fee_wallet);
        fee_config.set_ncn_fee_wallet(ncn_fee_wallet);

        fee_config.check_fees_okay(current_epoch)?;

        Ok(fee_config)
    }

    // ------------- GETTERS -------------

    /// Gets the currently active fee configuration for the given epoch
    /// Returns the fee configuration that should be used for calculations in the current epoch
    pub fn current_fees(&self, current_epoch: u64) -> &Fees {
        // If either fee is not yet active, return the other one
        if self.fee_1.activation_epoch() > current_epoch {
            return &self.fee_2;
        }
        if self.fee_2.activation_epoch() > current_epoch {
            return &self.fee_1;
        }

        // Otherwise return the one with higher activation epoch (most recent)
        if self.fee_1.activation_epoch() >= self.fee_2.activation_epoch() {
            &self.fee_1
        } else {
            &self.fee_2
        }
    }

    /// Gets the fee configuration that can be updated for future epochs
    /// Returns the fee configuration that is not currently active and can be modified
    fn updatable_fees(&mut self, current_epoch: u64) -> &mut Fees {
        // If either fee is scheduled for next epoch, return that one
        if self.fee_1.activation_epoch() > current_epoch {
            return &mut self.fee_1;
        }
        if self.fee_2.activation_epoch() > current_epoch {
            return &mut self.fee_2;
        }

        // Otherwise return the one with lower activation epoch (older one)
        if self.fee_1.activation_epoch() <= self.fee_2.activation_epoch() {
            &mut self.fee_1
        } else {
            &mut self.fee_2
        }
    }

    /// Updates the activation epoch of the updatable fee configuration to next epoch
    fn update_updatable_epoch(&mut self, current_epoch: u64) -> Result<(), NCNProgramError> {
        let next_epoch = current_epoch
            .checked_add(1)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let updatable_fees = self.updatable_fees(current_epoch);
        updatable_fees.set_activation_epoch(next_epoch);

        Ok(())
    }

    // ------------------- TOTAL FEES -------------------

    /// Gets the total fee percentage in basis points for the current epoch
    pub fn total_fees_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.total_fees_bps()
    }

    /// Gets the total fee percentage as a precise number for accurate calculations
    pub fn precise_total_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_total_fee_bps()
    }

    /// Gets the adjusted total fee percentage (currently same as total_fees_bps)
    /// This method exists for potential future adjustments based on other factors
    pub fn adjusted_total_fees_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let total_fees_bps = self.total_fees_bps(current_epoch)?;
        self.adjusted_fee_bps(
            total_fees_bps
                .try_into()
                .map_err(|_| NCNProgramError::ArithmeticOverflow)?,
        )
    }

    // ------------------- JITO DAO FEES -------------------

    /// Gets the Jito DAO fee percentage in basis points for the current epoch
    pub fn jito_dao_fee_bps(&self, current_epoch: u64) -> Result<u16, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.jito_dao_fee_bps()
    }

    /// Gets the Jito DAO fee percentage as a precise number for accurate calculations
    pub fn precise_jito_dao_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_jito_dao_fee_bps()
    }

    /// Gets the adjusted Jito DAO fee percentage (currently same as jito_dao_fee_bps)
    pub fn adjusted_jito_dao_fee_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.jito_dao_fee_bps()?;
        self.adjusted_fee_bps(fee)
    }

    /// Gets the adjusted Jito DAO fee percentage as a precise number
    pub fn adjusted_precise_jito_dao_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.jito_dao_fee_bps()?;
        self.adjusted_precise_fee_bps(fee)
    }

    /// Sets the Jito DAO fee percentage for the next epoch
    pub fn set_jito_dao_fee_bps(
        &mut self,
        value: u16,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        let updateable_fees = self.updatable_fees(current_epoch);
        updateable_fees.set_jito_dao_fee_bps(value)
    }

    // ------------------- NCN FEES -------------------

    /// Gets the NCN fee percentage in basis points for the current epoch
    pub fn ncn_fee_bps(&self, current_epoch: u64) -> Result<u16, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.ncn_fee_bps()
    }

    /// Gets the NCN fee percentage as a precise number for accurate calculations
    pub fn precise_ncn_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_ncn_fee_bps()
    }

    /// Gets the adjusted NCN fee percentage (currently same as ncn_fee_bps)
    pub fn adjusted_ncn_fee_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.ncn_fee_bps()?;
        self.adjusted_fee_bps(fee)
    }

    /// Gets the adjusted NCN fee percentage as a precise number
    pub fn adjusted_precise_ncn_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.ncn_fee_bps()?;
        self.adjusted_precise_fee_bps(fee)
    }

    /// Sets the NCN fee percentage for the next epoch
    pub fn set_ncn_fee_bps(
        &mut self,
        value: u16,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        let updateable_fees = self.updatable_fees(current_epoch);
        updateable_fees.set_ncn_fee_bps(value)
    }

    // ------------------- WALLETS -------------------

    /// Gets the NCN fee wallet address
    pub fn ncn_fee_wallet(&self) -> &Pubkey {
        &self.ncn_fee_wallets
    }

    /// Sets the NCN fee wallet address (takes effect immediately)
    pub fn set_ncn_fee_wallet(&mut self, wallet: &Pubkey) {
        self.ncn_fee_wallets = *wallet;
    }

    /// Gets the Jito DAO fee wallet address
    pub fn jito_dao_fee_wallet(&self) -> &Pubkey {
        &self.jito_dao_fee_wallet
    }

    /// Sets the Jito DAO fee wallet address (takes effect immediately)
    pub fn set_jito_dao_fee_wallet(&mut self, wallet: &Pubkey) {
        self.jito_dao_fee_wallet = *wallet;
    }

    // ------------- SETTERS -------------

    /// Copies the current active fees to the updatable fee configuration
    /// This is used when making changes to ensure we start from the current active state
    fn set_fees_to_current(&mut self, current_epoch: u64) -> Result<(), NCNProgramError> {
        if self.fee_1.activation_epoch() > current_epoch
            || self.fee_2.activation_epoch() > current_epoch
        {
            return Err(NCNProgramError::FeeNotActive);
        }

        let cloned_current_fees = *self.current_fees(current_epoch);
        let updatable_fees = self.updatable_fees(current_epoch);
        *updatable_fees = cloned_current_fees;

        Ok(())
    }

    /// Updates the Fee Configuration with new values
    ///
    /// This method handles the complex logic of updating fees while maintaining the
    /// epoch-delayed update system. Changes take effect in the next epoch, and wallets
    /// can be updated immediately.
    ///
    /// The process:
    /// 1. Copy current fees to updatable configuration if needed
    /// 2. Apply new fee values to updatable configuration  
    /// 3. Set activation epoch to next epoch
    /// 4. Validate all fees are within acceptable ranges
    #[allow(clippy::too_many_arguments)]
    pub fn update_fee_config(
        &mut self,
        new_jito_dao_fee_bps: Option<u16>,
        new_jito_dao_fee_wallet: Option<Pubkey>,
        new_ncn_fee_bps: Option<u16>,
        new_ncn_fee_wallet: Option<Pubkey>,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        // Copy current fees to updatable configuration if starting fresh
        {
            let updatable_fees = self.updatable_fees(current_epoch);
            if updatable_fees.activation_epoch() <= current_epoch {
                self.set_fees_to_current(current_epoch)?;
            }
        }

        // Update Jito DAO fee settings
        if let Some(new_jito_dao_fee_bps) = new_jito_dao_fee_bps {
            self.set_jito_dao_fee_bps(new_jito_dao_fee_bps, current_epoch)?;
        }

        if let Some(new_jito_dao_fee_wallet) = new_jito_dao_fee_wallet {
            self.set_jito_dao_fee_wallet(&new_jito_dao_fee_wallet);
        }

        // Update NCN fee settings
        if let Some(new_ncn_fee_bps) = new_ncn_fee_bps {
            self.set_ncn_fee_bps(new_ncn_fee_bps, current_epoch)?;
        }

        if let Some(new_ncn_fee_wallet) = new_ncn_fee_wallet {
            self.set_ncn_fee_wallet(&new_ncn_fee_wallet);
        }

        // Set activation epoch to next epoch
        self.update_updatable_epoch(current_epoch)?;

        // Validate fee configurations for current and next epoch
        self.check_fees_okay(current_epoch)?;
        self.check_fees_okay(
            current_epoch
                .checked_add(1)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        )?;

        Ok(())
    }

    // ------ HELPERS -----------------

    /// Validates that fee configuration is acceptable
    /// Checks that total fees don't exceed maximum and are greater than zero
    pub fn check_fees_okay(&self, current_epoch: u64) -> Result<(), NCNProgramError> {
        let _ = self.adjusted_precise_jito_dao_fee_bps(current_epoch)?;
        let _ = self.adjusted_precise_ncn_fee_bps(current_epoch)?;

        let total_fees_bps = self.total_fees_bps(current_epoch)?;
        if total_fees_bps > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        if total_fees_bps == 0 {
            return Err(NCNProgramError::TotalFeesCannotBeZero);
        }

        Ok(())
    }

    /// Adjusts fee value (currently a pass-through, but exists for future adjustments)
    /// This method exists to potentially adjust fees based on other factors in the future
    fn adjusted_fee_bps(&self, fee: u16) -> Result<u64, NCNProgramError> {
        // TODO: check this code
        // let remaining_bps = MAX_FEE_BPS
        //     .checked_sub(fee.jito_dao_fee_bps() as u64)
        //     .ok_or(NCNProgramError::ArithmeticUnderflowError)?;
        // (fee as u64)
        //     .checked_mul(MAX_FEE_BPS)
        //     .and_then(|x| x.checked_div(remaining_bps))
        // .ok_or(NCNProgramError::DenominatorIsZero)
        Ok(fee as u64)
    }

    /// Adjusts fee value as precise number (currently a pass-through)
    fn adjusted_precise_fee_bps(&self, fee: u16) -> Result<PreciseNumber, NCNProgramError> {
        // TODO: check this code
        // let remaining_bps = MAX_FEE_BPS
        //     .checked_sub(self.block_engine_fee_bps() as u64)
        //     .ok_or(NCNProgramError::ArithmeticOverflow)?;
        //
        // let precise_remaining_bps = PreciseNumber::new(remaining_bps as u128)
        //     .ok_or(NCNProgramError::NewPreciseNumberError)?;
        //
        // let adjusted_fee = (fee as u64)
        //     .checked_mul(MAX_FEE_BPS)
        //     .ok_or(NCNProgramError::ArithmeticOverflow)?;
        //
        // let precise_adjusted_fee = PreciseNumber::new(adjusted_fee as u128)
        //     .ok_or(NCNProgramError::NewPreciseNumberError)?;
        //
        // precise_adjusted_fee
        //     .checked_div(&precise_remaining_bps)
        //     .ok_or(NCNProgramError::DenominatorIsZero)

        Ok(PreciseNumber::new(fee as u128).ok_or(NCNProgramError::NewPreciseNumberError)?)
    }
}

/// Fee Configuration for a Specific Epoch
///
/// This struct represents the fee settings that are active during a specific epoch.
/// It contains the activation epoch and the actual fee values in basis points.
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct Fees {
    /// The epoch when these fees become active
    activation_epoch: PodU64,

    /// Jito DAO fee in basis points
    jito_dao_fee_bps: Fee,
    /// NCN fee in basis points  
    ncn_fee_bps: Fee,
}

impl Fees {
    /// Default fee values in basis points (400 = 4%)
    pub const JITO_DAO_FEE_BPS: u16 = 400;
    pub const NCN_DEFAULT_FEE_BPS: u16 = 400;

    /// Creates a new fee configuration for a specific epoch
    pub fn new(
        jito_dao_fee_bps: u16,
        default_ncn_fee_bps: u16,
        epoch: u64,
    ) -> Result<Self, NCNProgramError> {
        let mut fees = Self {
            activation_epoch: PodU64::from(epoch),
            jito_dao_fee_bps: Fee::default(),
            ncn_fee_bps: Fee::default(),
        };

        fees.set_jito_dao_fee_bps(jito_dao_fee_bps)?;
        fees.set_ncn_fee_bps(default_ncn_fee_bps)?;

        Ok(fees)
    }

    // ------ GETTERS -----------------

    /// Gets the epoch when these fees become active
    pub fn activation_epoch(&self) -> u64 {
        self.activation_epoch.into()
    }

    /// Gets the Jito DAO fee in basis points
    pub fn jito_dao_fee_bps(&self) -> Result<u16, NCNProgramError> {
        Ok(self.jito_dao_fee_bps.fee())
    }

    /// Gets the Jito DAO fee as a precise number for calculations
    pub fn precise_jito_dao_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let fee = self.jito_dao_fee_bps()?;

        PreciseNumber::new(fee.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

    /// Gets the NCN fee in basis points
    pub fn ncn_fee_bps(&self) -> Result<u16, NCNProgramError> {
        Ok(self.ncn_fee_bps.fee())
    }

    /// Gets the NCN fee as a precise number for calculations
    pub fn precise_ncn_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let fee = self.ncn_fee_bps()?;

        PreciseNumber::new(fee.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

    /// Calculates the total fees in basis points (sum of all individual fees)
    pub fn total_fees_bps(&self) -> Result<u64, NCNProgramError> {
        let mut total_fee_bps: u64 = 0;

        let jito_dao_fee_bps = self.jito_dao_fee_bps()?;
        total_fee_bps = total_fee_bps
            .checked_add(jito_dao_fee_bps as u64)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let ncn_fee_bps = self.ncn_fee_bps()?;

        total_fee_bps = total_fee_bps
            .checked_add(ncn_fee_bps as u64)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        Ok(total_fee_bps)
    }

    /// Gets the total fees as a precise number for accurate calculations
    pub fn precise_total_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let total_fee_bps = self.total_fees_bps()?;
        PreciseNumber::new(total_fee_bps.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

    // ------ SETTERS -----------------

    /// Sets the activation epoch for these fees
    fn set_activation_epoch(&mut self, value: u64) {
        self.activation_epoch = PodU64::from(value);
    }

    /// Sets the Jito DAO fee with validation
    pub fn set_jito_dao_fee_bps(&mut self, value: u16) -> Result<(), NCNProgramError> {
        if value as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        self.jito_dao_fee_bps = Fee::new(value);

        Ok(())
    }

    /// Sets the NCN fee with validation
    pub fn set_ncn_fee_bps(&mut self, value: u16) -> Result<(), NCNProgramError> {
        if value as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        self.ncn_fee_bps = Fee::new(value);

        Ok(())
    }
}

/// Individual Fee Value Wrapper
///
/// This struct wraps a fee value to provide type safety and encapsulation.
/// It exists because we can't use PodU16 directly in nested structs in some contexts.
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct Fee {
    /// Fee value in basis points
    fee: PodU16,
}

impl Default for Fee {
    fn default() -> Self {
        Self {
            fee: PodU16::from(0),
        }
    }
}

impl Fee {
    /// Creates a new fee with the specified value
    pub fn new(fee: u16) -> Self {
        Self {
            fee: PodU16::from(fee),
        }
    }

    /// Gets the fee value in basis points
    pub fn fee(&self) -> u16 {
        self.fee.into()
    }
}

#[cfg(test)]
mod tests {
    use solana_program::pubkey::Pubkey;

    use super::*;

    /// Tests basic fee configuration creation and retrieval
    /// Validates that fees are properly initialized and can be retrieved correctly
    #[test]
    fn test_get_all_fees() {
        const JITO_DAO_FEE: u16 = 200;
        const DEFAULT_NCN_FEE: u16 = 300;
        const STARTING_EPOCH: u64 = 10;

        let dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        let fee_config = FeeConfig::new(
            &dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            STARTING_EPOCH,
        )
        .unwrap();

        fee_config.check_fees_okay(STARTING_EPOCH).unwrap();

        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH).unwrap(),
            JITO_DAO_FEE
        );

        assert_eq!(*fee_config.ncn_fee_wallet(), ncn_fee_wallet);

        assert_eq!(fee_config.fee_1.jito_dao_fee_bps().unwrap(), JITO_DAO_FEE);
        assert_eq!(fee_config.fee_2.jito_dao_fee_bps().unwrap(), JITO_DAO_FEE);

        assert_eq!(fee_config.fee_1.ncn_fee_bps().unwrap(), DEFAULT_NCN_FEE);
        assert_eq!(fee_config.fee_2.ncn_fee_bps().unwrap(), DEFAULT_NCN_FEE);
    }

    /// Tests various error conditions during fee configuration initialization
    /// Validates that proper errors are returned for invalid inputs
    #[test]
    fn test_init_fee_config_errors() {
        const OK_FEE: u16 = 1;
        const ZERO_FEE: u16 = 0;
        const OK_EPOCH: u64 = 0;

        let ok_wallet = Pubkey::new_unique();

        // Test rejection of default (zero) DAO wallet
        let error = FeeConfig::new(&Pubkey::default(), OK_FEE, &ok_wallet, OK_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::DefaultDaoWallet);

        // Test rejection of default (zero) NCN wallet
        let error = FeeConfig::new(&ok_wallet, OK_FEE, &Pubkey::default(), OK_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::DefaultNcnWallet);

        // Test rejection of excessive Jito DAO fee
        let error = FeeConfig::new(
            &ok_wallet,
            (MAX_FEE_BPS as u16) + 1,
            &ok_wallet,
            OK_FEE,
            OK_EPOCH,
        );
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // Test rejection of excessive NCN fee
        let error = FeeConfig::new(
            &ok_wallet,
            OK_FEE,
            &ok_wallet,
            (MAX_FEE_BPS as u16) + 1,
            OK_EPOCH,
        );
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // Test rejection of zero total fees
        let error = FeeConfig::new(&ok_wallet, ZERO_FEE, &ok_wallet, ZERO_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::TotalFeesCannotBeZero);

        // Test rejection when total fees exceed maximum
        let error = FeeConfig::new(&ok_wallet, MAX_FEE_BPS as u16, &ok_wallet, 1, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);
    }

    /// Tests the epoch-delayed fee update mechanism
    /// Validates that fees update on the next epoch while wallets update immediately
    #[test]
    fn test_update_fees() {
        const JITO_DAO_FEE: u16 = 200;
        const NEW_JITO_DAO_FEE: u16 = 600;
        const NEW_NEW_JITO_DAO_FEE: u16 = 800;
        const DEFAULT_NCN_FEE: u16 = 300;
        const NEW_DEFAULT_NCN_FEE: u16 = 700;
        const NEW_NEW_DEFAULT_NCN_FEE: u16 = 900;
        const STARTING_EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let new_jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();
        let new_ncn_fee_wallet = Pubkey::new_unique();

        let mut fee_config = FeeConfig::new(
            &jito_dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            STARTING_EPOCH,
        )
        .unwrap();

        // Apply first round of updates
        fee_config
            .update_fee_config(
                Some(NEW_JITO_DAO_FEE),
                Some(new_jito_dao_fee_wallet),
                Some(NEW_DEFAULT_NCN_FEE),
                Some(new_ncn_fee_wallet),
                STARTING_EPOCH,
            )
            .unwrap();

        // Verify wallets update immediately
        assert_eq!(*fee_config.jito_dao_fee_wallet(), new_jito_dao_fee_wallet);
        assert_eq!(*fee_config.ncn_fee_wallet(), new_ncn_fee_wallet);

        // Verify fees update on next epoch (epoch-delayed)
        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH).unwrap(),
            JITO_DAO_FEE
        );
        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH + 1).unwrap(),
            NEW_JITO_DAO_FEE
        );
        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH).unwrap(),
            DEFAULT_NCN_FEE
        );
        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH + 1).unwrap(),
            NEW_DEFAULT_NCN_FEE
        );

        // Test second round of updates (from next epoch)
        fee_config
            .update_fee_config(
                Some(NEW_NEW_JITO_DAO_FEE),
                None,
                Some(NEW_NEW_DEFAULT_NCN_FEE),
                None,
                STARTING_EPOCH + 1,
            )
            .unwrap();

        // Verify wallet remains unchanged (None passed)
        assert_eq!(*fee_config.jito_dao_fee_wallet(), new_jito_dao_fee_wallet);

        // Verify fee progression across epochs
        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH + 1).unwrap(),
            NEW_JITO_DAO_FEE
        );
        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH + 2).unwrap(),
            NEW_NEW_JITO_DAO_FEE
        );

        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH + 1).unwrap(),
            NEW_DEFAULT_NCN_FEE
        );
        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH + 2).unwrap(),
            NEW_NEW_DEFAULT_NCN_FEE
        );
    }

    /// Tests that fee updates with no changes work correctly
    /// Validates that calling update with None values doesn't break anything
    #[test]
    fn test_update_fees_no_change() {
        const JITO_DAO_FEE: u16 = 200;
        const DEFAULT_NCN_FEE: u16 = 300;
        const STARTING_EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        let mut fee_config = FeeConfig::new(
            &jito_dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            STARTING_EPOCH,
        )
        .unwrap();

        // Call update with no changes
        fee_config
            .update_fee_config(None, None, None, None, STARTING_EPOCH)
            .unwrap();

        // Verify nothing changed
        assert_eq!(*fee_config.jito_dao_fee_wallet(), jito_dao_fee_wallet);
        assert_eq!(*fee_config.ncn_fee_wallet(), ncn_fee_wallet);

        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH).unwrap(),
            JITO_DAO_FEE
        );
        assert_eq!(
            fee_config.jito_dao_fee_bps(STARTING_EPOCH + 1).unwrap(),
            JITO_DAO_FEE
        );

        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH).unwrap(),
            DEFAULT_NCN_FEE
        );
        assert_eq!(
            fee_config.ncn_fee_bps(STARTING_EPOCH + 1).unwrap(),
            DEFAULT_NCN_FEE
        );
    }

    /// Tests that valid fee configurations pass validation
    #[test]
    fn test_check_fees_okay() {
        const JITO_DAO_FEE: u16 = 200;
        const DEFAULT_NCN_FEE: u16 = 300;
        const STARTING_EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        let fee_config = FeeConfig::new(
            &jito_dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            STARTING_EPOCH,
        )
        .unwrap();

        fee_config.check_fees_okay(STARTING_EPOCH).unwrap();
    }

    /// Tests that invalid fee configurations are properly rejected
    /// Validates error handling for fees that exceed limits
    #[test]
    fn test_check_fees_not_okay() {
        const JITO_DAO_FEE: u16 = 200;
        const DEFAULT_NCN_FEE: u16 = 300;
        const STARTING_EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        let mut fee_config = FeeConfig::new(
            &jito_dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            STARTING_EPOCH,
        )
        .unwrap();

        fee_config.check_fees_okay(STARTING_EPOCH).unwrap();

        // Test rejection of excessive Jito DAO fee
        let result = fee_config.update_fee_config(
            Some((MAX_FEE_BPS as u16) + 1),
            None,
            None,
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // Test rejection of excessive NCN fee
        let result = fee_config.update_fee_config(
            None,
            None,
            Some((MAX_FEE_BPS as u16) + 1),
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // Test rejection when total fees exceed maximum
        let result = fee_config.update_fee_config(
            Some(MAX_FEE_BPS as u16),
            None,
            Some(1),
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);
    }

    /// Tests the current fee selection logic across epoch transitions
    /// Validates that the system correctly selects active vs. future fees
    #[test]
    fn test_current_fee() {
        let mut fee_config =
            FeeConfig::new(&Pubkey::new_unique(), 100, &Pubkey::new_unique(), 200, 5).unwrap();

        // Initially both fees have activation epoch 5
        assert_eq!(fee_config.current_fees(5).activation_epoch(), 5);

        // Set fee_1 to activate in the future
        fee_config.fee_1.set_activation_epoch(10);

        // Should still use fee_2 (epoch 5) when current epoch < 10
        assert_eq!(fee_config.current_fees(5).activation_epoch(), 5);
        // Should use fee_1 (epoch 10) when current epoch >= 10
        assert_eq!(fee_config.current_fees(10).activation_epoch(), 10);

        // Set fee_2 to activate even further in the future
        fee_config.fee_2.set_activation_epoch(15);

        // Should use fee_1 between epochs 10-14
        assert_eq!(fee_config.current_fees(12).activation_epoch(), 10);
        // Should use fee_2 from epoch 15 onwards
        assert_eq!(fee_config.current_fees(15).activation_epoch(), 15);
    }

    /// Tests the updatable fee selection logic
    /// Validates which fee configuration can be modified for future epochs
    #[test]
    fn test_get_updatable_fee_mut() {
        let mut fee_config =
            FeeConfig::new(&Pubkey::new_unique(), 100, &Pubkey::new_unique(), 200, 5).unwrap();

        // Modify fee_1 for future activation
        let fees = fee_config.updatable_fees(10);
        fees.set_jito_dao_fee_bps(400).unwrap();
        fees.set_activation_epoch(11);

        // Verify fee_1 was modified
        assert_eq!(fee_config.fee_1.jito_dao_fee_bps().unwrap(), 400);
        assert_eq!(fee_config.fee_1.activation_epoch(), 11);

        // Set fee_2 for even further future
        fee_config.fee_2.set_activation_epoch(13);

        // Should now select fee_2 as updatable (lower activation epoch)
        let fees = fee_config.updatable_fees(12);
        fees.set_jito_dao_fee_bps(500).unwrap();
        fees.set_activation_epoch(13);

        // Verify fee_2 was modified
        assert_eq!(fee_config.fee_2.jito_dao_fee_bps().unwrap(), 500);
        assert_eq!(fee_config.fee_2.activation_epoch(), 13);

        // When current epoch is very high, should pick older activation epoch
        assert_eq!(fee_config.updatable_fees(u64::MAX).activation_epoch(), 11);
    }

    /// Tests precise fee calculations
    /// Validates that PreciseNumber calculations work correctly for total fees
    #[test]
    fn test_precise_total_fee_bps() {
        // Setup test values
        const JITO_DAO_FEE: u16 = 200;
        const DEFAULT_NCN_FEE: u16 = 300;
        const EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        // Create fee config
        let fee_config = FeeConfig::new(
            &jito_dao_fee_wallet,
            JITO_DAO_FEE,
            &ncn_fee_wallet,
            DEFAULT_NCN_FEE,
            EPOCH,
        )
        .unwrap();

        // Test precise total calculation
        let total = fee_config.precise_total_fee_bps(EPOCH).unwrap();
        let expected = PreciseNumber::new((JITO_DAO_FEE + DEFAULT_NCN_FEE) as u128).unwrap();

        assert!(total.eq(&expected));
    }

    /// Tests precise Jito DAO fee calculation
    #[test]
    fn test_precise_jito_dao_fee_bps() {
        const JITO_DAO_FEE: u16 = 100;
        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();

        let fee_config =
            FeeConfig::new(&jito_dao_fee_wallet, JITO_DAO_FEE, &ncn_fee_wallet, 0, 0).unwrap();

        let precise_fee = fee_config.precise_jito_dao_fee_bps(0).unwrap();
        let expected = PreciseNumber::new(JITO_DAO_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

    /// Tests NCN fee retrieval
    #[test]
    fn test_ncn_fee_bps() {
        const NCN_FEE: u16 = 300;
        const EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();
        let fee_config =
            FeeConfig::new(&jito_dao_fee_wallet, 0, &ncn_fee_wallet, NCN_FEE, EPOCH).unwrap();

        let fee = fee_config.ncn_fee_bps(EPOCH).unwrap();
        assert_eq!(fee, NCN_FEE);
    }

    /// Tests precise NCN fee calculation
    #[test]
    fn test_precise_ncn_fee_bps() {
        const NCN_FEE: u16 = 300;
        const EPOCH: u64 = 10;

        let jito_dao_fee_wallet = Pubkey::new_unique();
        let ncn_fee_wallet = Pubkey::new_unique();
        let fee_config =
            FeeConfig::new(&jito_dao_fee_wallet, 0, &ncn_fee_wallet, NCN_FEE, EPOCH).unwrap();

        let precise_fee = fee_config.precise_ncn_fee_bps(EPOCH).unwrap();
        let expected = PreciseNumber::new(NCN_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

    /// Tests precise Jito DAO fee calculation at the Fees level
    #[test]
    fn test_fees_precise_jito_dao_fee_bps() {
        const JITO_DAO_FEE: u16 = 200;

        let fees = Fees::new(JITO_DAO_FEE, 0, 0).unwrap();

        let precise_fee = fees.precise_jito_dao_fee_bps().unwrap();
        let expected = PreciseNumber::new(JITO_DAO_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

    /// Tests precise NCN fee calculation at the Fees level
    #[test]
    fn test_fees_precise_ncn_fee_bps() {
        const NCN_FEE: u16 = 300;

        let fees = Fees::new(0, NCN_FEE, 0).unwrap();

        let precise_fee = fees.precise_ncn_fee_bps().unwrap();
        let expected = PreciseNumber::new(NCN_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

    /// Tests precise total fee calculation at the Fees level
    #[test]
    fn test_fees_precise_total_fee_bps() {
        const JITO_DAO_FEE: u16 = 200;
        const NCN_FEE: u16 = 300;

        let fees = Fees::new(JITO_DAO_FEE, NCN_FEE, 0).unwrap();

        let precise_total = fees.precise_total_fee_bps().unwrap();
        let expected = PreciseNumber::new((JITO_DAO_FEE + NCN_FEE) as u128).unwrap();

        assert!(precise_total.eq(&expected));
    }
}
