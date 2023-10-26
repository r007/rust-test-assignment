<p align="center">
  <a href="https://solana.com">
    <img alt="Solana" src="https://i.imgur.com/uBVzyX3.png" width="250" />
  </a>
</p>

# Test Assignment for SFXDX

This project is written in Rust to showcase the very simple solana smart contract. In this demo store users can buy and sell SPL tokens for SOL (solana currency). The purchase/sale price is determined by the store itself and stored in the separate account in solana network.

One store can sell one SPL token at a fixed price. Smart contract should have the following instructions:

- **initialize_store** initialization of the store's account.
- **update_price** update the price of a token.
- **sell** tokens for SOL.
- **buy** tokens for SOL.
