use solana_program::pubkey::Pubkey;

pub mod entrypoint;
pub mod instruction;
pub mod processor;
pub mod state;

solana_program::declare_id!("ArK3BKiUg7dbU4pNQuWZ73arMp6uunSuuBjzm4gSwk8T");

pub const ITEM_METADATA_SEED: &[u8] = b"item_metadata";
pub const ITEM_SEED: &[u8] = b"item";

pub fn find_item_metadata_address(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ITEM_METADATA_SEED, &mint.to_bytes()], &id())
}

pub fn find_item_address(mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ITEM_SEED, &mint.to_bytes()], &id())
}

#[cfg(test)]
mod tests {
    use borsh::*;
    #[allow(unused)]
    use pretty_assertions::{assert_eq, assert_ne};
    use solana_program_test::{tokio, BanksClient};
    use solana_sdk::{account::Account, hash::Hash, signature::Keypair, system_program};

    #[test]
    fn instruction_unpack() {
        use crate::instruction::{Args, Payload, FixedPriceSaleInstruction};
        use solana_program::program_error::ProgramError;

        let args = Args {
            lamports: Some(1000000000),
            metadata_bump: None,
        };

        let payload = to_vec(&Payload {
            instruction: 0,
            args,
        })
        .unwrap();

        let invalid_payload = to_vec(&Payload {
            instruction: 3,
            args,
        })
        .unwrap();

        let unpacked = FixedPriceSaleInstruction::unpack(&payload);
        let invalid_unpacked = FixedPriceSaleInstruction::unpack(&invalid_payload);

        assert_eq!(unpacked, Ok((FixedPriceSaleInstruction::Sell, args)));
        assert_eq!(invalid_unpacked, Err(ProgramError::InvalidInstructionData));
    }

    async fn program_test_setup() -> (BanksClient, Keypair, Hash, (Keypair, Keypair, Keypair)) {
        use solana_program_test::{processor, ProgramTest};
        use solana_sdk::{
            program_pack::*, signer::Signer, system_instruction::create_account,
            sysvar::rent::Rent, transaction::Transaction,
        };
        use spl_associated_token_account::{
            get_associated_token_address, instruction::create_associated_token_account,
        };
        use spl_token::{
            instruction::{initialize_mint2, mint_to, set_authority, AuthorityType},
            state::Mint,
        };

        let mut program_test = ProgramTest::new(
            "fixed_price_sale",
            crate::id(),
            processor!(crate::processor::instruction_processor),
        );

        let buyer = Keypair::new();

        program_test.add_account(
            buyer.pubkey(),
            Account {
                lamports: 10000000000,
                data: vec![],
                owner: system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        let (mut bank, payer, hash) = program_test.start().await;

        let rent = Rent::default();
        let mint = Keypair::new();
        let payment = Keypair::new();

        let space = Mint::get_packed_len();
        let rent_lamports = rent.minimum_balance(space);

        let payer_item_wallet = get_associated_token_address(&payer.pubkey(), &mint.pubkey());
        let buyer_payment_wallet = get_associated_token_address(&buyer.pubkey(), &payment.pubkey());

        let setup_instructions = [
            // Initialize SPL token
            create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                rent_lamports,
                space.try_into().unwrap(),
                &spl_token::id(),
            ),
            initialize_mint2(&spl_token::id(), &mint.pubkey(), &payer.pubkey(), None, 0).unwrap(),
            create_associated_token_account(
                &payer.pubkey(),
                &payer.pubkey(),
                &mint.pubkey(),
                &spl_token::id(),
            ),
            create_associated_token_account(
                &payer.pubkey(),
                &buyer.pubkey(),
                &mint.pubkey(),
                &spl_token::id(),
            ),
            mint_to(
                &spl_token::id(),
                &mint.pubkey(),
                &payer_item_wallet,
                &payer.pubkey(),
                &[],
                1,
            )
            .unwrap(),
            set_authority(
                &spl_token::id(),
                &mint.pubkey(),
                None,
                AuthorityType::MintTokens,
                &payer.pubkey(),
                &[],
            )
            .unwrap(),
            // Initialize payment token
            create_account(
                &payer.pubkey(),
                &payment.pubkey(),
                rent_lamports,
                space.try_into().unwrap(),
                &spl_token::id(),
            ),
            initialize_mint2(
                &spl_token::id(),
                &payment.pubkey(),
                &payer.pubkey(),
                None,
                9,
            )
            .unwrap(),
            create_associated_token_account(
                &payer.pubkey(),
                &payer.pubkey(),
                &payment.pubkey(),
                &spl_token::id(),
            ),
            create_associated_token_account(
                &payer.pubkey(),
                &buyer.pubkey(),
                &payment.pubkey(),
                &spl_token::id(),
            ),
            mint_to(
                &spl_token::id(),
                &payment.pubkey(),
                &buyer_payment_wallet,
                &payer.pubkey(),
                &[],
                200000000,
            )
            .unwrap(),
        ];

        let mut transaction =
            Transaction::new_with_payer(&setup_instructions, Some(&payer.pubkey()));
        transaction.sign(&[&payer, &mint, &payment], hash);

        bank.process_transaction(transaction).await.unwrap();

        let hash = bank.get_latest_blockhash().await.unwrap();

        (bank, payer, hash, (mint, payment, buyer))
    }

    #[tokio::test]
    async fn processor_sell() {
        use solana_sdk::{signer::Signer, transaction::Transaction};
        use spl_associated_token_account::{
            get_associated_token_address, instruction::create_associated_token_account,
        };
        use spl_token::{instruction::transfer, state::Account};

        let (mut bank, payer, hash, (mint, payment, _)) = program_test_setup().await;

        let (item_addr, _) = &crate::find_item_address(&mint.pubkey());

        let payer_item_wallet = get_associated_token_address(&payer.pubkey(), &mint.pubkey());
        let payer_payment_wallet = get_associated_token_address(&payer.pubkey(), &payment.pubkey());

        let program_item_wallet = get_associated_token_address(item_addr, &mint.pubkey());

        let sell_price = 200000000;

        let sell_instructions = [
            create_associated_token_account(
                &payer.pubkey(),
                item_addr,
                &mint.pubkey(),
                &spl_token::id(),
            ),
            transfer(
                &spl_token::id(),
                &payer_item_wallet,
                &program_item_wallet,
                &payer.pubkey(),
                &[],
                1,
            )
            .unwrap(),
            crate::instruction::sell(
                &payer.pubkey(),
                &program_item_wallet,
                &mint.pubkey(),
                &payer_payment_wallet,
                sell_price,
            ),
        ];

        let mut transaction =
            Transaction::new_with_payer(&sell_instructions, Some(&payer.pubkey()));
        transaction.sign(&[&payer], hash);

        bank.process_transaction(transaction).await.unwrap();

        let (item_metadata_addr, _) = crate::find_item_metadata_address(&mint.pubkey());

        let payer_item_wallet_data: Account = bank
            .get_packed_account_data(payer_item_wallet)
            .await
            .unwrap();
        let program_item_wallet_data: Account = bank
            .get_packed_account_data(program_item_wallet)
            .await
            .unwrap();
        let item_metadata_data: crate::state::ItemMetadata = bank
            .get_account_data_with_borsh(item_metadata_addr)
            .await
            .unwrap();

        assert_eq!(payer_item_wallet_data.amount, 0);
        assert_eq!(program_item_wallet_data.amount, 1);

        assert_eq!(item_metadata_data.lamports, sell_price);
        assert_eq!(item_metadata_data.payment, payer_payment_wallet);
        assert_eq!(item_metadata_data.seller, payer.pubkey());
        assert_eq!(item_metadata_data.item, program_item_wallet);
        assert_eq!(item_metadata_data.mint, mint.pubkey());
    }

    #[tokio::test]
    async fn processor_buy() {
        use solana_sdk::{signer::Signer, transaction::Transaction};
        use spl_associated_token_account::{
            get_associated_token_address, instruction::create_associated_token_account,
        };
        use spl_token::{self, instruction::transfer, state::Account};

        let (mut bank, payer, hash, (mint, payment, buyer)) = program_test_setup().await;

        let sell_price = 200000000;

        let (item_addr, _) = &crate::find_item_address(&mint.pubkey());
        let (item_metadata_addr, _) = crate::find_item_metadata_address(&mint.pubkey());

        let payer_item_wallet = get_associated_token_address(&payer.pubkey(), &mint.pubkey());
        let payer_payment_wallet = get_associated_token_address(&payer.pubkey(), &payment.pubkey());

        let buyer_item_wallet = get_associated_token_address(&buyer.pubkey(), &mint.pubkey());
        let buyer_payment_wallet = get_associated_token_address(&buyer.pubkey(), &payment.pubkey());

        let program_item_wallet = get_associated_token_address(&item_addr, &mint.pubkey());

        let sell_instructions = [
            create_associated_token_account(
                &payer.pubkey(),
                &item_addr,
                &mint.pubkey(),
                &spl_token::id(),
            ),
            transfer(
                &spl_token::id(),
                &payer_item_wallet,
                &program_item_wallet,
                &payer.pubkey(),
                &[],
                1,
            )
            .unwrap(),
            crate::instruction::sell(
                &payer.pubkey(),
                &program_item_wallet,
                &mint.pubkey(),
                &payer_payment_wallet,
                sell_price,
            ),
            crate::instruction::buy(
                &buyer.pubkey(),
                &buyer_payment_wallet,
                &buyer_item_wallet,
                &program_item_wallet,
                &payer_payment_wallet,
                &item_metadata_addr,
                &item_addr,
            ),
        ];

        let mut transaction =
            Transaction::new_with_payer(&sell_instructions, Some(&payer.pubkey()));
        transaction.sign(&[&payer, &buyer], hash);

        bank.process_transaction(transaction).await.unwrap();

        let payer_item_wallet_data: Account = bank
            .get_packed_account_data(payer_item_wallet)
            .await
            .unwrap();
        let program_item_wallet_data: Account = bank
            .get_packed_account_data(program_item_wallet)
            .await
            .unwrap();
        let buyer_item_wallet_data: Account = bank
            .get_packed_account_data(buyer_item_wallet)
            .await
            .unwrap();

        let payer_payment_wallet_data: Account = bank
            .get_packed_account_data(payer_payment_wallet)
            .await
            .unwrap();
        let buyer_payment_wallet_data: Account = bank
            .get_packed_account_data(buyer_payment_wallet)
            .await
            .unwrap();

        let item_metadata_data = bank.get_account(item_metadata_addr).await.unwrap();

        assert_eq!(payer_item_wallet_data.amount, 0);
        assert_eq!(program_item_wallet_data.amount, 0);
        assert_eq!(buyer_item_wallet_data.amount, 1);

        assert_eq!(buyer_payment_wallet_data.amount, 0);
        assert_eq!(payer_payment_wallet_data.amount, sell_price);

        assert_eq!(item_metadata_data, None);
    }
}
