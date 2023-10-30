use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(Debug, BorshDeserialize, BorshSerialize)]
pub struct ItemMetadata {
    /// Seller address
    pub seller: Pubkey,
    /// Mint ID of selling token
    pub mint: Pubkey,
    /// Selling price
    pub lamports: u64,
    /// Associated token account, used to accept payment
    pub payment: Pubkey,
    /// Associated SPL token account
    pub item: Pubkey,
}
