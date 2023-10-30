use borsh::{to_vec, BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_option::COption,
    program_pack::*,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction::create_account,
    sysvar::Sysvar,
};
use spl_token::{
    check_program_account,
    instruction::transfer,
    state::{Account, Mint},
};

use crate::{
    instruction::{Args, FixedPriceSaleInstruction},
    state::ItemMetadata,
};

pub fn instruction_processor(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (instruction, args) = FixedPriceSaleInstruction::unpack(instruction_data)?;

    Ok(match instruction {
        FixedPriceSaleInstruction::Sell => sell(program_id, accounts, args)?,
        FixedPriceSaleInstruction::Buy => buy(program_id, accounts)?,
    })
}

fn sell(program_id: &Pubkey, accounts: &[AccountInfo], args: Args) -> ProgramResult {
    // Create an iterator to safely reference accounts in the slice
    let accounts_info_iter = &mut accounts.iter();

    // As part of the program specification the instruction gives:
    let seller = next_account_info(accounts_info_iter)?; // 1.
    let program_item_wallet = next_account_info(accounts_info_iter)?; // 2.
    let mint = next_account_info(accounts_info_iter)?; // 3.
    let item_metadata = next_account_info(accounts_info_iter)?; // 4.
    let seller_payment_wallet = next_account_info(accounts_info_iter)?; // 5.
    let _sys_program = next_account_info(accounts_info_iter)?; // 6.

    check_program_account(mint.owner)?;
    check_program_account(program_item_wallet.owner)?;
    check_program_account(seller_payment_wallet.owner)?;

    // Validate non-fungible-token
    if {
        let data = Mint::unpack(*mint.data.borrow())?;
        data.mint_authority != COption::None
            || data.supply != 1
            || !data.is_initialized
            || data.decimals != 0
    } {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify transfered token
    if {
        let (item_addr, _) = crate::find_item_address(mint.key);
        let data = Account::unpack(*program_item_wallet.data.borrow())?;
        data.owner != item_addr
            || data.amount != 1
            || data.mint != *mint.key
            || !data.is_initialized()
    } {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate payment token
    if {
        let data = Account::unpack(*seller_payment_wallet.data.borrow())?;
        !data.is_initialized()
    } {
        return Err(ProgramError::UninitializedAccount);
    }

    let rent = Rent::get()?;

    let metadata = ItemMetadata {
        seller: seller.key.clone(),
        mint: mint.key.clone(),
        lamports: args.lamports.unwrap(),
        payment: seller_payment_wallet.key.clone(),
        item: program_item_wallet.key.clone(),
    };

    let space = to_vec(&metadata).unwrap().len();
    let rent_lamports = rent.minimum_balance(space);

    msg!("Attempting to sell SPL token...");
    invoke_signed(
        &create_account(
            seller.key,
            item_metadata.key,
            rent_lamports,
            space.try_into().unwrap(),
            program_id,
        ),
        &[seller.clone(), item_metadata.clone()],
        &[&[
            crate::ITEM_METADATA_SEED,
            &mint.key.to_bytes(),
            &[args.metadata_bump.unwrap()],
        ]],
    )?;

    metadata.serialize(&mut *item_metadata.data.borrow_mut())?;

    msg!(
        "Mint {} sold! Metadata is created at {}",
        mint.key,
        item_metadata.key
    );

    Ok(())
}

fn buy(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    // Create an iterator to safely reference accounts in the slice
    let accounts_info_iter = &mut accounts.iter();

    // As part of the program specification the instruction gives:
    let buyer = next_account_info(accounts_info_iter)?; // 1.
    let buyer_payment_wallet = next_account_info(accounts_info_iter)?; // 2.
    let buyer_item_wallet = next_account_info(accounts_info_iter)?; // 3.
    let program_item_wallet = next_account_info(accounts_info_iter)?; // 4.
    let seller_payment_wallet = next_account_info(accounts_info_iter)?; // 5.
    let item_metadata = next_account_info(accounts_info_iter)?; // 6.
    let spl_token = next_account_info(accounts_info_iter)?; // 7.
    let program_item = next_account_info(accounts_info_iter)?; // 8.

    if item_metadata.owner != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    let metadata_data = ItemMetadata::try_from_slice(*item_metadata.data.borrow())?;

    // Transfer from buyer to seller
    invoke(
        &transfer(
            spl_token.key,
            buyer_payment_wallet.key,
            &metadata_data.payment,
            buyer.key,
            &[],
            metadata_data.lamports,
        )?,
        &[
            buyer_payment_wallet.clone(),
            seller_payment_wallet.clone(),
            buyer.clone(),
        ],
    )?;

    let (item_addr, item_bump) = crate::find_item_address(&metadata_data.mint);

    // Transfer SPL token from store to buyer
    invoke_signed(
        &transfer(
            spl_token.key,
            &metadata_data.item,
            buyer_item_wallet.key,
            &item_addr,
            &[],
            1,
        )?,
        &[
            program_item_wallet.clone(),
            buyer_item_wallet.clone(),
            program_item.clone(),
        ],
        &[&[
            crate::ITEM_SEED,
            &metadata_data.mint.to_bytes(),
            &[item_bump],
        ]],
    )?;

    // Destroy metadata
    let metadata_lamports = item_metadata.lamports();

    **buyer.lamports.borrow_mut() = buyer.lamports().checked_add(metadata_lamports).unwrap();
    **item_metadata.lamports.borrow_mut() = 0;

    item_metadata.data.borrow_mut().fill(0);

    msg!("Bought {}", metadata_data.mint);

    Ok(())
}
