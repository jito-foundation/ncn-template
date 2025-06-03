use std::fmt;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about = "A CLI for creating and managing the ncn program", long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: ProgramCommand,

    #[arg(
        long,
        global = true,
        env = "RPC_URL",
        default_value = "https://api.mainnet-beta.solana.com",
        help = "RPC URL to use"
    )]
    pub rpc_url: String,

    #[arg(
        long,
        global = true,
        env = "COMMITMENT",
        default_value = "confirmed",
        help = "Commitment level"
    )]
    pub commitment: String,

    #[arg(
        long,
        global = true,
        env = "PRIORITY_FEE_MICRO_LAMPORTS",
        default_value_t = 1,
        help = "Priority fee in micro lamports"
    )]
    pub priority_fee_micro_lamports: u64,

    #[arg(
        long,
        global = true,
        env = "TRANSACTION_RETRIES",
        default_value_t = 0,
        help = "Amount of times to retry a transaction"
    )]
    pub transaction_retries: u64,

    #[arg(
        long,
        global = true,
        env = "NCN_PROGRAM_ID",
        default_value_t = ncn_program::id().to_string(),
        help = "NCN program ID"
    )]
    pub ncn_program_id: String,

    #[arg(
        long,
        global = true,
        env = "RESTAKING_PROGRAM_ID",
        default_value_t = jito_restaking_program::id().to_string(),
        help = "Restaking program ID"
    )]
    pub restaking_program_id: String,

    #[arg(
        long,
        global = true,
        env = "VAULT_PROGRAM_ID", 
        default_value_t = jito_vault_program::id().to_string(),
        help = "Vault program ID"
    )]
    pub vault_program_id: String,

    #[arg(
        long,
        global = true,
        env = "TOKEN_PROGRAM_ID",
        default_value_t = spl_token::id().to_string(),
        help = "Token Program ID"
    )]
    pub token_program_id: String,

    #[arg(long, global = true, env = "NCN", help = "NCN Account Address")]
    pub ncn: Option<String>,

    #[arg(
        long,
        global = true,
        env = "OPERATOR",
        help = "Operator Account Address"
    )]
    pub operator: Option<String>,

    #[arg(
        long,
        global = true,
        env = "EPOCH",
        help = "Epoch - defaults to current epoch"
    )]
    pub epoch: Option<u64>,

    #[arg(long, global = true, env = "KEYPAIR_PATH", help = "keypair path")]
    pub keypair_path: Option<String>,

    #[arg(long, global = true, help = "Verbose mode")]
    pub verbose: bool,

    #[arg(long, global = true, hide = true)]
    pub markdown_help: bool,

    #[arg(
        long,
        global = true,
        env = "OPENWEATHER_API_KEY",
        help = "Open weather api key"
    )]
    pub open_weather_api_key: Option<String>,
}

#[derive(Subcommand)]
pub enum ProgramCommand {
    /// Keeper
    Keeper {
        #[arg(
            long,
            env,
            default_value_t = 600_000, // 10 minutes
            help = "Keeper error timeout in milliseconds"
        )]
        loop_timeout_ms: u64,
        #[arg(
            long,
            env,
            default_value_t = 10_000, // 10 seconds
            help = "Keeper error timeout in milliseconds"
        )]
        error_timeout_ms: u64,
        #[arg(long, help = "Calls test vote, instead of waiting for a real vote")]
        test_vote: bool,
        #[arg(long, env, help = "Only emit metrics")]
        metrics_only: bool,
        #[arg(long, env, help = "Cluster label for metrics purposes")]
        cluster: Cluster,
        #[arg(
            long,
            env,
            default_value = "local",
            help = "Region for metrics purposes"
        )]
        region: String,
    },
    /// Instructions
    OperatorCastVote {
        #[arg(long, help = "Weather status at solana beach")]
        weather_status: u8,
    },

    /// Getters
    GetNcn,
    GetNcnOperatorState,
    GetVaultNcnTicket {
        #[arg(long, env = "VAULT", help = "Vault Account Address")]
        vault: String,
    },
    GetNcnVaultTicket {
        #[arg(long, env = "VAULT", help = "Vault Account Address")]
        vault: String,
    },
    GetVaultOperatorDelegation {
        #[arg(long, env = "VAULT", help = "Vault Account Address")]
        vault: String,
    },
    GetAllTickets,
    GetAllOperatorsInNcn,
    GetAllVaultsInNcn,
    GetNCNProgramConfig,
    GetVaultRegistry,
    GetWeightTable,
    GetEpochState,
    GetEpochSnapshot,
    GetOperatorSnapshot,
    GetBallotBox,
    GetAccountPayer,
    GetTotalEpochRentCost,
    GetConsensusResult,

    GetOperatorStakes,
    GetVaultStakes,
    GetVaultOperatorStakes,
}

#[rustfmt::skip]
impl fmt::Display for Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n NCN Program CLI Configuration")?;
        writeln!(f, "═══════════════════════════════════════")?;

        // Network Configuration
        writeln!(f, "\n📡 Network Settings:")?;
        writeln!(f, "  • RPC URL:     {}", self.rpc_url)?;
        writeln!(f, "  • Commitment:  {}", self.commitment)?;

        // Program IDs
        writeln!(f, "\n🔑 Program IDs:")?;
        writeln!(f, "  • NCN Program:        {}", self.ncn_program_id)?;
        writeln!(f, "  • Restaking:         {}", self.restaking_program_id)?;
        writeln!(f, "  • Vault:             {}", self.vault_program_id)?;
        writeln!(f, "  • Token:             {}", self.token_program_id)?;

        // Solana Settings
        writeln!(f, "\n◎  Solana Settings:")?;
        writeln!(f, "  • Keypair Path:  {}", self.keypair_path.as_deref().unwrap_or("Not Set"))?;
        writeln!(f, "  • NCN:  {}", self.ncn.as_deref().unwrap_or("Not Set"))?;
        writeln!(f, "  • Epoch: {}", if self.epoch.is_some() { format!("{}", self.epoch.unwrap()) } else { "Current".to_string() })?;

        // Optional Settings
        writeln!(f, "\n⚙️  Additional Settings:")?;
        writeln!(f, "  • Verbose Mode:  {}", if self.verbose { "Enabled" } else { "Disabled" })?;
        writeln!(f, "  • Markdown Help: {}", if self.markdown_help { "Enabled" } else { "Disabled" })?;

        writeln!(f, "\n")?;

        Ok(())
    }
}

#[derive(ValueEnum, Debug, Clone)]
pub enum Cluster {
    Mainnet,
    Testnet,
    Localnet,
}

impl fmt::Display for Cluster {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mainnet => write!(f, "mainnet"),
            Self::Testnet => write!(f, "testnet"),
            Self::Localnet => write!(f, "localnet"),
        }
    }
}
