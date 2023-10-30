use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

#[derive(Debug, PartialEq)]
pub enum FixedPriceSaleInstruction {
    Sell,
    Buy,
}

impl FixedPriceSaleInstruction {
    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, Args), ProgramError> {
        let payload = Payload::try_from_slice(instruction_data)?;

        let instruction = match payload.instruction {
            0 => Self::Sell,
            1 => Self::Buy,
            _ => return Err(ProgramError::InvalidInstructionData),
        };

        Ok((instruction, payload.args))
    }
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct Payload {
    pub instruction: u8,
    pub args: Args,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Clone, Copy, Debug)]
pub struct Args {
    // The amount in lamports that will be paid
    pub lamports: Option<u64>,
    pub metadata_bump: Option<u8>,
}

pub fn sell(
    seller: &Pubkey,
    program_item_wallet: &Pubkey,
    mint: &Pubkey,
    seller_payment_wallet: &Pubkey,
    lamports: u64,
) -> Instruction {
    let (item_metadata_addr, item_metadata_bump) = crate::find_item_metadata_address(mint);

    Instruction::new_with_borsh(
        crate::id(),
        &Payload {
            instruction: 0, // Sell
            args: Args {
                lamports: Some(lamports),
                metadata_bump: Some(item_metadata_bump),
            },
        },
        vec![
            AccountMeta::new_readonly(*seller, true),
            AccountMeta::new_readonly(*program_item_wallet, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(item_metadata_addr, false),
            AccountMeta::new_readonly(*seller_payment_wallet, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}

pub fn buy(
    buyer: &Pubkey,
    buyer_payment_wallet: &Pubkey,
    buyer_item_wallet: &Pubkey,
    program_item_wallet: &Pubkey,
    seller_payment_wallet: &Pubkey,
    item_metadata: &Pubkey,
    program_item: &Pubkey,
) -> Instruction {
    Instruction::new_with_borsh(
        crate::id(),
        &Payload {
            instruction: 1, // Buy
            args: Args {
                lamports: None,
                metadata_bump: None,
            },
        },
        vec![
            AccountMeta::new(*buyer, true),
            AccountMeta::new(*buyer_payment_wallet, false),
            AccountMeta::new(*buyer_item_wallet, false),
            AccountMeta::new(*program_item_wallet, false),
            AccountMeta::new(*seller_payment_wallet, false),
            AccountMeta::new(*item_metadata, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(*program_item, false),
        ],
    )
}
