use bytemuck::{Pod, Zeroable};
use jito_bytemuck::types::{PodU16, PodU64};
use shank::ShankType;
use solana_program::pubkey::Pubkey;
use spl_math::precise_number::PreciseNumber;

use crate::{constants::MAX_FEE_BPS, error::NCNProgramError};

/// Fee Config. Allows for fee updates to take place in a future epoch without requiring an update.
/// This is important so all operators calculate the same Merkle root regardless of when fee changes take place.
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct FeeConfig {
    jito_dao_fee_wallet: Pubkey,
    jito_dao_fee_bps: PodU16,

    ncn_fee_wallets: Pubkey,
    ncn_fee_bps: PodU16,

    // Two fees so that we can update one and use the other, on the epoch boundary we switch
    /// Fee 1
    fee_1: Fees,
    /// Fee 2
    fee_2: Fees,
}

impl FeeConfig {
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

    // ------------- Getters -------------
    pub fn current_fees(&self, current_epoch: u64) -> &Fees {
        // If either fee is not yet active, return the other one
        if self.fee_1.activation_epoch() > current_epoch {
            return &self.fee_2;
        }
        if self.fee_2.activation_epoch() > current_epoch {
            return &self.fee_1;
        }

        // Otherwise return the one with higher activation epoch
        if self.fee_1.activation_epoch() >= self.fee_2.activation_epoch() {
            &self.fee_1
        } else {
            &self.fee_2
        }
    }

    fn updatable_fees(&mut self, current_epoch: u64) -> &mut Fees {
        // If either fee is scheduled for next epoch, return that one
        if self.fee_1.activation_epoch() > current_epoch {
            return &mut self.fee_1;
        }
        if self.fee_2.activation_epoch() > current_epoch {
            return &mut self.fee_2;
        }

        // Otherwise return the one with lower activation epoch
        if self.fee_1.activation_epoch() <= self.fee_2.activation_epoch() {
            &mut self.fee_1
        } else {
            &mut self.fee_2
        }
    }

    fn update_updatable_epoch(&mut self, current_epoch: u64) -> Result<(), NCNProgramError> {
        let next_epoch = current_epoch
            .checked_add(1)
            .ok_or(NCNProgramError::ArithmeticOverflow)?;

        let updatable_fees = self.updatable_fees(current_epoch);
        updatable_fees.set_activation_epoch(next_epoch);

        Ok(())
    }

    // ------------------- TOTALS -------------------
    pub fn total_fees_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.total_fees_bps()
    }

    pub fn precise_total_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_total_fee_bps()
    }

    pub fn adjusted_total_fees_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let total_fees_bps = self.total_fees_bps(current_epoch)?;
        self.adjusted_fee_bps(
            total_fees_bps
                .try_into()
                .map_err(|_| NCNProgramError::ArithmeticOverflow)?,
        )
    }

    // ------------------- JITO DAO FEES -------------------

    pub fn jito_dao_fee_bps(&self, current_epoch: u64) -> Result<u16, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.jito_dao_fee_bps()
    }

    pub fn precise_jito_dao_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_jito_dao_fee_bps()
    }

    pub fn adjusted_jito_dao_fee_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.jito_dao_fee_bps()?;
        self.adjusted_fee_bps(fee)
    }

    pub fn adjusted_precise_jito_dao_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.jito_dao_fee_bps()?;
        self.adjusted_precise_fee_bps(fee)
    }

    pub fn set_jito_dao_fee_bps(
        &mut self,
        value: u16,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        let updateable_fees = self.updatable_fees(current_epoch);
        updateable_fees.set_jito_dao_fee_bps(value)
    }

    // ------------------- NCN FEES -------------------

    pub fn ncn_fee_bps(&self, current_epoch: u64) -> Result<u16, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.ncn_fee_bps()
    }

    pub fn precise_ncn_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        current_fees.precise_ncn_fee_bps()
    }

    pub fn adjusted_ncn_fee_bps(&self, current_epoch: u64) -> Result<u64, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.ncn_fee_bps()?;
        self.adjusted_fee_bps(fee)
    }

    pub fn adjusted_precise_ncn_fee_bps(
        &self,
        current_epoch: u64,
    ) -> Result<PreciseNumber, NCNProgramError> {
        let current_fees = self.current_fees(current_epoch);
        let fee = current_fees.ncn_fee_bps()?;
        self.adjusted_precise_fee_bps(fee)
    }

    pub fn set_ncn_fee_bps(
        &mut self,
        value: u16,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        let updateable_fees = self.updatable_fees(current_epoch);
        updateable_fees.set_ncn_fee_bps(value)
    }

    // ------------------- WALLETS -------------------

    pub fn ncn_fee_wallet(&self) -> &Pubkey {
        &self.ncn_fee_wallets
    }

    pub fn set_ncn_fee_wallet(&mut self, wallet: &Pubkey) {
        self.ncn_fee_wallets = *wallet;
    }

    pub fn jito_dao_fee_wallet(&self) -> &Pubkey {
        &self.jito_dao_fee_wallet
    }

    pub fn set_jito_dao_fee_wallet(&mut self, wallet: &Pubkey) {
        self.jito_dao_fee_wallet = *wallet;
    }

    // ------------- Setters -------------

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

    /// Updates the Fee Config
    #[allow(clippy::too_many_arguments)]
    pub fn update_fee_config(
        &mut self,
        new_jito_dao_fee_bps: Option<u16>,
        new_jito_dao_fee_wallet: Option<Pubkey>,
        new_ncn_fee_bps: Option<u16>,
        new_ncn_fee_wallet: Option<Pubkey>,
        current_epoch: u64,
    ) -> Result<(), NCNProgramError> {
        // IF NEW CHANGES, COPY OVER CURRENT FEES
        {
            let updatable_fees = self.updatable_fees(current_epoch);
            if updatable_fees.activation_epoch() <= current_epoch {
                self.set_fees_to_current(current_epoch)?;
            }
        }

        // JITO DAO FEE
        if let Some(new_jito_dao_fee_bps) = new_jito_dao_fee_bps {
            self.set_jito_dao_fee_bps(new_jito_dao_fee_bps, current_epoch)?;
        }

        if let Some(new_jito_dao_fee_wallet) = new_jito_dao_fee_wallet {
            self.set_jito_dao_fee_wallet(&new_jito_dao_fee_wallet);
        }

        // NCN FEE
        if let Some(new_ncn_fee_bps) = new_ncn_fee_bps {
            self.set_ncn_fee_bps(new_ncn_fee_bps, current_epoch)?;
        }

        if let Some(new_ncn_fee_wallet) = new_ncn_fee_wallet {
            self.set_ncn_fee_wallet(&new_ncn_fee_wallet);
        }

        // ACTIVATION EPOCH
        self.update_updatable_epoch(current_epoch)?;

        // CHECK FEES
        self.check_fees_okay(current_epoch)?;
        self.check_fees_okay(
            current_epoch
                .checked_add(1)
                .ok_or(NCNProgramError::ArithmeticOverflow)?,
        )?;

        Ok(())
    }

    // ------ Helpers -----------------

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

    fn adjusted_fee_bps(&self, fee: u16) -> Result<u64, NCNProgramError> {
        // let remaining_bps = MAX_FEE_BPS
        //     .checked_sub(fee.jito_dao_fee_bps() as u64)
        //     .ok_or(NCNProgramError::ArithmeticUnderflowError)?;
        // (fee as u64)
        //     .checked_mul(MAX_FEE_BPS)
        //     .and_then(|x| x.checked_div(remaining_bps))
        // .ok_or(NCNProgramError::DenominatorIsZero)
        Ok(fee as u64)
    }

    fn adjusted_precise_fee_bps(&self, fee: u16) -> Result<PreciseNumber, NCNProgramError> {
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

#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct Fees {
    activation_epoch: PodU64,

    jito_dao_fee_bps: Fee,
    ncn_fee_bps: Fee,
}

impl Fees {
    pub const JITO_DAO_FEE_BPS: u16 = 400;
    pub const NCN_DEFAULT_FEE_BPS: u16 = 400;

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

    // ------ Getters -----------------
    pub fn activation_epoch(&self) -> u64 {
        self.activation_epoch.into()
    }

    pub fn jito_dao_fee_bps(&self) -> Result<u16, NCNProgramError> {
        Ok(self.jito_dao_fee_bps.fee())
    }

    pub fn precise_jito_dao_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let fee = self.jito_dao_fee_bps()?;

        PreciseNumber::new(fee.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

    pub fn ncn_fee_bps(&self) -> Result<u16, NCNProgramError> {
        Ok(self.ncn_fee_bps.fee())
    }

    pub fn precise_ncn_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let fee = self.ncn_fee_bps()?;

        PreciseNumber::new(fee.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

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

    pub fn precise_total_fee_bps(&self) -> Result<PreciseNumber, NCNProgramError> {
        let total_fee_bps = self.total_fees_bps()?;
        PreciseNumber::new(total_fee_bps.into()).ok_or(NCNProgramError::NewPreciseNumberError)
    }

    // ------ Setters -----------------
    fn set_activation_epoch(&mut self, value: u64) {
        self.activation_epoch = PodU64::from(value);
    }

    pub fn set_jito_dao_fee_bps(&mut self, value: u16) -> Result<(), NCNProgramError> {
        if value as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        self.jito_dao_fee_bps = Fee::new(value);

        Ok(())
    }

    pub fn set_ncn_fee_bps(&mut self, value: u16) -> Result<(), NCNProgramError> {
        if value as u64 > MAX_FEE_BPS {
            return Err(NCNProgramError::FeeCapExceeded);
        }

        self.ncn_fee_bps = Fee::new(value);

        Ok(())
    }
}

// ----------- FEE Because we can't do PodU16 in struct ------------
#[derive(Debug, Clone, Copy, Zeroable, ShankType, Pod)]
#[repr(C)]
pub struct Fee {
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
    pub fn new(fee: u16) -> Self {
        Self {
            fee: PodU16::from(fee),
        }
    }

    pub fn fee(&self) -> u16 {
        self.fee.into()
    }
}

#[cfg(test)]
mod tests {
    use solana_program::pubkey::Pubkey;

    use super::*;

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

    #[test]
    fn test_init_fee_config_errors() {
        const OK_FEE: u16 = 1;
        const ZERO_FEE: u16 = 0;
        const OK_EPOCH: u64 = 0;

        let ok_wallet = Pubkey::new_unique();

        // DEFAULT DAO WALLET
        let error = FeeConfig::new(&Pubkey::default(), OK_FEE, &ok_wallet, OK_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::DefaultDaoWallet);

        // DEFAULT NCN WALLET
        let error = FeeConfig::new(&ok_wallet, OK_FEE, &Pubkey::default(), OK_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::DefaultNcnWallet);

        // JITO DAO FEE
        let error = FeeConfig::new(
            &ok_wallet,
            (MAX_FEE_BPS as u16) + 1,
            &ok_wallet,
            OK_FEE,
            OK_EPOCH,
        );
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // NCN FEE
        let error = FeeConfig::new(
            &ok_wallet,
            OK_FEE,
            &ok_wallet,
            (MAX_FEE_BPS as u16) + 1,
            OK_EPOCH,
        );
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);

        // TotalFeesCannotBeZero
        let error = FeeConfig::new(&ok_wallet, ZERO_FEE, &ok_wallet, ZERO_FEE, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::TotalFeesCannotBeZero);

        // total fees overflow
        let error = FeeConfig::new(&ok_wallet, MAX_FEE_BPS as u16, &ok_wallet, 1, OK_EPOCH);
        assert_eq!(error.err().unwrap(), NCNProgramError::FeeCapExceeded);
    }

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

        fee_config
            .update_fee_config(
                Some(NEW_JITO_DAO_FEE),
                Some(new_jito_dao_fee_wallet),
                Some(NEW_DEFAULT_NCN_FEE),
                Some(new_ncn_fee_wallet),
                STARTING_EPOCH,
            )
            .unwrap();

        // Wallets update immediately
        assert_eq!(*fee_config.jito_dao_fee_wallet(), new_jito_dao_fee_wallet);
        assert_eq!(*fee_config.ncn_fee_wallet(), new_ncn_fee_wallet);

        // Fees update on next epoch
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

        // test update again
        fee_config
            .update_fee_config(
                Some(NEW_NEW_JITO_DAO_FEE),
                None,
                Some(NEW_NEW_DEFAULT_NCN_FEE),
                None,
                STARTING_EPOCH + 1,
            )
            .unwrap();

        assert_eq!(*fee_config.jito_dao_fee_wallet(), new_jito_dao_fee_wallet);

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

        fee_config
            .update_fee_config(None, None, None, None, STARTING_EPOCH)
            .unwrap();

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

        let result = fee_config.update_fee_config(
            Some((MAX_FEE_BPS as u16) + 1),
            None,
            None,
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);

        let result = fee_config.update_fee_config(
            None,
            None,
            Some((MAX_FEE_BPS as u16) + 1),
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);

        let result = fee_config.update_fee_config(
            Some(MAX_FEE_BPS as u16),
            None,
            Some(1),
            None,
            STARTING_EPOCH,
        );
        assert_eq!(result.err().unwrap(), NCNProgramError::FeeCapExceeded);
    }

    #[test]
    fn test_current_fee() {
        let mut fee_config =
            FeeConfig::new(&Pubkey::new_unique(), 100, &Pubkey::new_unique(), 200, 5).unwrap();

        assert_eq!(fee_config.current_fees(5).activation_epoch(), 5);

        fee_config.fee_1.set_activation_epoch(10);

        assert_eq!(fee_config.current_fees(5).activation_epoch(), 5);
        assert_eq!(fee_config.current_fees(10).activation_epoch(), 10);

        fee_config.fee_2.set_activation_epoch(15);

        assert_eq!(fee_config.current_fees(12).activation_epoch(), 10);
        assert_eq!(fee_config.current_fees(15).activation_epoch(), 15);
    }

    #[test]
    fn test_get_updatable_fee_mut() {
        let mut fee_config =
            FeeConfig::new(&Pubkey::new_unique(), 100, &Pubkey::new_unique(), 200, 5).unwrap();

        let fees = fee_config.updatable_fees(10);
        fees.set_jito_dao_fee_bps(400).unwrap();
        fees.set_activation_epoch(11);

        assert_eq!(fee_config.fee_1.jito_dao_fee_bps().unwrap(), 400);
        assert_eq!(fee_config.fee_1.activation_epoch(), 11);

        fee_config.fee_2.set_activation_epoch(13);

        let fees = fee_config.updatable_fees(12);
        fees.set_jito_dao_fee_bps(500).unwrap();
        fees.set_activation_epoch(13);

        assert_eq!(fee_config.fee_2.jito_dao_fee_bps().unwrap(), 500);
        assert_eq!(fee_config.fee_2.activation_epoch(), 13);

        assert_eq!(fee_config.updatable_fees(u64::MAX).activation_epoch(), 11);
    }

    #[test]
    fn test_precise_total_fee_bps() {
        // Setup
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

        // Test the function
        let total = fee_config.precise_total_fee_bps(EPOCH).unwrap();
        let expected = PreciseNumber::new((JITO_DAO_FEE + DEFAULT_NCN_FEE) as u128).unwrap();

        assert!(total.eq(&expected));
    }

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

    // #[test]
    // fn test_adjusted_jito_dao_fee_bps() {
    //     const JITO_DAO_FEE: u16 = 200;
    //     const EPOCH: u64 = 10;

    //     let jito_dao_fee_wallet = Pubkey::new_unique();
    //     let ncn_fee_wallet = Pubkey::new_unique();
    //     let fee_config = FeeConfig::new(
    //         &jito_dao_fee_wallet,
    //         JITO_DAO_FEE,
    //         &ncn_fee_wallet,
    //         0,
    //         EPOCH,
    //     )
    //     .unwrap();

    //     let adjusted_fee = fee_config.adjusted_jito_dao_fee_bps(EPOCH).unwrap();

    //     let expected = JITO_DAO_FEE as u64;
    //     assert_eq!(adjusted_fee, expected);
    // }

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

    // #[test]
    // fn test_adjusted_ncn_fee_bps() {
    //     const BLOCK_ENGINE_FEE: u16 = 100;
    //     const NCN_FEE: u16 = 300;
    //     const EPOCH: u64 = 10;

    //     let dao_fee_wallet = Pubkey::new_unique();
    //     let fee_config =
    //         FeeConfig::new(&dao_fee_wallet, BLOCK_ENGINE_FEE, 0, NCN_FEE, EPOCH).unwrap();

    //     let ncn_fee_group = NcnFeeGroup::default();
    //     let adjusted_fee = fee_config
    //         .adjusted_ncn_fee_bps(ncn_fee_group, EPOCH)
    //         .unwrap();

    //     // Expected calculation: NCN_FEE * MAX_FEE_BPS / (MAX_FEE_BPS - BLOCK_ENGINE_FEE)
    //     let expected = ((NCN_FEE as f64 * MAX_FEE_BPS as f64)
    //         / (MAX_FEE_BPS as f64 - (BLOCK_ENGINE_FEE as f64)).trunc())
    //         as u64;
    //     assert_eq!(adjusted_fee, expected);
    // }

    #[test]
    fn test_fees_precise_jito_dao_fee_bps() {
        const JITO_DAO_FEE: u16 = 200;

        let fees = Fees::new(JITO_DAO_FEE, 0, 0).unwrap();

        let precise_fee = fees.precise_jito_dao_fee_bps().unwrap();
        let expected = PreciseNumber::new(JITO_DAO_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

    #[test]
    fn test_fees_precise_ncn_fee_bps() {
        const NCN_FEE: u16 = 300;

        let fees = Fees::new(0, NCN_FEE, 0).unwrap();

        let precise_fee = fees.precise_ncn_fee_bps().unwrap();
        let expected = PreciseNumber::new(NCN_FEE.into()).unwrap();

        assert!(precise_fee.eq(&expected));
    }

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
