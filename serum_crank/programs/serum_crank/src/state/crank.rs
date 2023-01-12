use {
    anchor_lang::{prelude::*, AnchorDeserialize},
    std::convert::TryFrom,
};

pub const SEED_CRANK: &[u8] = b"crank";

/**
 * Crank
 */

#[account]
#[derive(Debug)]
pub struct Crank {
    pub authority: Pubkey,
    pub event_queue: Pubkey,
    pub id: String,
    pub limit: u16,
    pub market: Pubkey,
    pub mint_a_vault: Pubkey,
    pub mint_a_wallet: Pubkey,
    pub mint_b_vault: Pubkey,
    pub mint_b_wallet: Pubkey,
    pub open_orders: Vec<Pubkey>,
    pub vault_signer: Pubkey,
}

impl Crank {
    pub fn pubkey(authority: Pubkey, market: Pubkey, id: String) -> Pubkey {
        Pubkey::find_program_address(
            &[
                SEED_CRANK,
                authority.as_ref(),
                market.as_ref(),
                id.as_bytes(),
            ],
            &crate::ID,
        )
        .0
    }
}

impl TryFrom<Vec<u8>> for Crank {
    type Error = Error;
    fn try_from(data: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        Crank::try_deserialize(&mut data.as_slice())
    }
}

/**
 * CrankAccount
 */

pub trait CrankAccount {
    fn new(
        &mut self,
        authority: Pubkey,
        event_queue: Pubkey,
        id: String,
        limit: u16,
        market: Pubkey,
        mint_a_vault: Pubkey,
        mint_a_wallet: Pubkey,
        mint_b_vault: Pubkey,
        mint_b_wallet: Pubkey,
        vault_signer: Pubkey,
    ) -> Result<()>;
}

impl CrankAccount for Account<'_, Crank> {
    fn new(
        &mut self,
        authority: Pubkey,
        event_queue: Pubkey,
        id: String,
        limit: u16,
        market: Pubkey,
        mint_a_vault: Pubkey,
        mint_a_wallet: Pubkey,
        mint_b_vault: Pubkey,
        mint_b_wallet: Pubkey,
        vault_signer: Pubkey,
    ) -> Result<()> {
        self.authority = authority;
        self.open_orders = Vec::new();
        self.id = id;
        self.market = market;
        self.event_queue = event_queue;
        self.mint_a_vault = mint_a_vault;
        self.mint_a_wallet = mint_a_wallet;
        self.mint_b_vault = mint_b_vault;
        self.mint_b_wallet = mint_b_wallet;
        self.limit = limit;
        self.vault_signer = vault_signer;
        Ok(())
    }
}
