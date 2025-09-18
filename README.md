# SolStakeNFT

SolStakeNFT is a Solana program for NFT staking, built using the Anchor framework. This project allows users to stake their NFTs and earn rewards, providing a foundation for NFT-based DeFi applications on the Solana blockchain.

## Features

- Stake and unstake NFTs securely
- Track staked NFT ownership and rewards
- Built with Anchor for safety and developer productivity
- Modular and extensible for custom reward logic

## Project Structure

```
SolStakeNFT/
├── Anchor.toml           # Anchor configuration
├── Cargo.toml            # Workspace manifest
├── programs/
│   └── nft-staking/
│       ├── Cargo.toml    # Program manifest
│       └── src/
│           └── lib.rs    # Main program logic
└── target/               # Build artifacts
```

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- [Anchor CLI](https://project-serum.github.io/anchor/getting-started/installation.html)

### Installation

1. Clone the repository:
   ```sh
   git clone https://github.com/yourusername/SolStakeNFT.git
   cd SolStakeNFT
   ```
2. Install dependencies:
   ```sh
   anchor build
   ```

### Building the Program

To build the SolStakeNFT program:

```sh
anchor build
```

### Deploying to Localnet

Start a local Solana validator and deploy the program:

```sh
solana-test-validator
# In a new terminal:
anchor deploy
```

### Testing

Run tests using Anchor:

```sh
anchor test
```

## Usage

- Interact with the program using Anchor scripts or your own client.
- Integrate with frontends or bots to allow users to stake/unstake NFTs.

## Program Details

- **Location:** `programs/nft-staking/src/lib.rs`
- **Entry Point:** Anchor's `#[program]` macro in `lib.rs`
- **Customization:** Extend reward logic or add new instructions as needed.

## Contributing

Pull requests are welcome! For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)

## Acknowledgements

- [Solana](https://solana.com/)
- [Anchor](https://project-serum.github.io/anchor/)
- [Metaplex](https://www.metaplex.com/)
