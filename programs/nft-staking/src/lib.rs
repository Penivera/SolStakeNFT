use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount},
};

declare_id!("9VhnzKxMtB8FzrqMJPMqE7rUUGnE7AMAYPLmYaUq4j2a");

#[program]
pub mod nft_staking {
    use super::*;

    pub fn initialize_collection(
        ctx: Context<InitializeCollection>,
        max_supply: u64,
    ) -> Result<()> {
        let collection_account = &mut ctx.accounts.collection_account;
        collection_account.authority = ctx.accounts.authority.key();
        collection_account.max_supply = max_supply;
        collection_account.current_supply = 0;
        collection_account.reward_mint = ctx.accounts.reward_mint.key();
        collection_account.reward_mint_bump = ctx.bumps.reward_mint;
        collection_account.total_staked = 0;
        collection_account.rewards_per_token_stored = 0;
        collection_account.last_update_time = Clock::get()?.unix_timestamp;
        collection_account.bump = ctx.bumps.collection_account;
        
        msg!("Collection initialized with max supply: {}", max_supply);
        Ok(())
    }

    pub fn mint_simple_nft(
        ctx: Context<MintSimpleNft>,
    ) -> Result<()> {
        let collection_account = &mut ctx.accounts.collection_account;
        
        require!(
            collection_account.current_supply < collection_account.max_supply,
            StakingError::MaxSupplyReached
        );
        
        require!(
            ctx.accounts.authority.key() == collection_account.authority,
            StakingError::NotOwner
        );

        // Mint the NFT token
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.token_account.to_account_info(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            1,
        )?;

        collection_account.current_supply += 1;
        
        msg!("NFT minted successfully. Current supply: {}", collection_account.current_supply);
        Ok(())
    }

    pub fn stake_nft(ctx: Context<StakeNft>) -> Result<()> {
        let collection_account = &mut ctx.accounts.collection_account;
        let stake_account = &mut ctx.accounts.stake_account;
        let clock = Clock::get()?;

        // Update rewards before staking
        update_rewards(collection_account, clock.unix_timestamp)?;

        // Transfer NFT to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.vault_token_account.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            1,
        )?;

        // Initialize stake account
        stake_account.user = ctx.accounts.user.key();
        stake_account.mint = ctx.accounts.mint.key();
        stake_account.stake_time = clock.unix_timestamp;
        stake_account.rewards_per_token_paid = collection_account.rewards_per_token_stored;
        stake_account.pending_rewards = 0;
        stake_account.bump = ctx.bumps.stake_account;

        collection_account.total_staked += 1;
        
        msg!("NFT staked successfully");
        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let collection_account = &mut ctx.accounts.collection_account;
        let stake_account = &mut ctx.accounts.stake_account;
        let clock = Clock::get()?;

        // Update rewards
        update_rewards(collection_account, clock.unix_timestamp)?;

        // Calculate pending rewards
        let rewards_earned = calculate_rewards(stake_account, collection_account)?;
        
        if rewards_earned > 0 {
            // Mint reward tokens to user
            let seeds = &[
                b"reward_mint",
                collection_account.to_account_info().key.as_ref(),
                &[collection_account.reward_mint_bump],
            ];
            let signer = &[&seeds[..]];

            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::MintTo {
                        mint: ctx.accounts.reward_mint.to_account_info(),
                        to: ctx.accounts.user_reward_account.to_account_info(),
                        authority: ctx.accounts.reward_mint.to_account_info(),
                    },
                    signer,
                ),
                rewards_earned,
            )?;

            stake_account.rewards_per_token_paid = collection_account.rewards_per_token_stored;
            stake_account.pending_rewards = 0;

            msg!("Rewards claimed: {}", rewards_earned);
        }

        Ok(())
    }

    pub fn unstake_nft(ctx: Context<UnstakeNft>) -> Result<()> {
        let collection_account = &mut ctx.accounts.collection_account;
        let stake_account = &ctx.accounts.stake_account;
        let clock = Clock::get()?;

        // Update and claim any pending rewards
        update_rewards(collection_account, clock.unix_timestamp)?;
        let rewards_earned = calculate_rewards(stake_account, collection_account)?;
        
        if rewards_earned > 0 {
            // Mint reward tokens to user
            let seeds = &[
                b"reward_mint",
                collection_account.to_account_info().key.as_ref(),
                &[collection_account.reward_mint_bump],
            ];
            let signer = &[&seeds[..]];

            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::MintTo {
                        mint: ctx.accounts.reward_mint.to_account_info(),
                        to: ctx.accounts.user_reward_account.to_account_info(),
                        authority: ctx.accounts.reward_mint.to_account_info(),
                    },
                    signer,
                ),
                rewards_earned,
            )?;
        }

        // Transfer NFT back to user
        let vault_seeds = &[
            b"vault",
            ctx.accounts.mint.to_account_info().key.as_ref(),
            &[ctx.accounts.vault_bump],
        ];
        let vault_signer = &[&vault_seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ctx.accounts.vault_token_account.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.vault_token_account.to_account_info(),
                },
                vault_signer,
            ),
            1,
        )?;

        collection_account.total_staked -= 1;
        
        msg!("NFT unstaked successfully");
        Ok(())
    }
}

// Helper functions
fn update_rewards(collection_account: &mut CollectionAccount, current_time: i64) -> Result<()> {
    if collection_account.total_staked > 0 {
        let time_diff = current_time - collection_account.last_update_time;
        let reward_rate = 100; // 100 tokens per second per NFT
        let rewards_per_token = (time_diff as u64 * reward_rate) / collection_account.total_staked;
        collection_account.rewards_per_token_stored += rewards_per_token;
    }
    collection_account.last_update_time = current_time;
    Ok(())
}

fn calculate_rewards(stake_account: &StakeAccount, collection_account: &CollectionAccount) -> Result<u64> {
    let rewards_per_token_diff = collection_account.rewards_per_token_stored - stake_account.rewards_per_token_paid;
    Ok(stake_account.pending_rewards + rewards_per_token_diff)
}

// Account structs
#[derive(Accounts)]
pub struct InitializeCollection<'info> {
    #[account(
        init,
        payer = payer,
        space = CollectionAccount::LEN,
        seeds = [b"collection"],
        bump
    )]
    pub collection_account: Account<'info, CollectionAccount>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = reward_mint,
        seeds = [b"reward_mint", collection_account.key().as_ref()],
        bump
    )]
    pub reward_mint: Account<'info, Mint>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct MintSimpleNft<'info> {
    #[account(
        mut,
        seeds = [b"collection"],
        bump = collection_account.bump
    )]
    pub collection_account: Account<'info, CollectionAccount>,

    #[account(
        init,
        payer = payer,
        mint::decimals = 0,
        mint::authority = authority
    )]
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = authority
    )]
    pub token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct StakeNft<'info> {
    #[account(
        mut,
        seeds = [b"collection"],
        bump = collection_account.bump
    )]
    pub collection_account: Account<'info, CollectionAccount>,

    #[account(
        init,
        payer = user,
        space = StakeAccount::LEN,
        seeds = [b"stake", user.key().as_ref(), mint.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = user,
        token::mint = mint,
        token::authority = vault_token_account,
        seeds = [b"vault", mint.key().as_ref()],
        bump
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
        mut,
        seeds = [b"collection"],
        bump = collection_account.bump
    )]
    pub collection_account: Account<'info, CollectionAccount>,

    #[account(
        mut,
        seeds = [b"stake", user.key().as_ref(), mint.key().as_ref()],
        bump = stake_account.bump,
        has_one = user
    )]
    pub stake_account: Account<'info, StakeAccount>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [b"reward_mint", collection_account.key().as_ref()],
        bump = collection_account.reward_mint_bump
    )]
    pub reward_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = reward_mint,
        associated_token::authority = user
    )]
    pub user_reward_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct UnstakeNft<'info> {
    #[account(
        mut,
        seeds = [b"collection"],
        bump = collection_account.bump
    )]
    pub collection_account: Account<'info, CollectionAccount>,

    #[account(
        mut,
        seeds = [b"stake", user.key().as_ref(), mint.key().as_ref()],
        bump = stake_account.bump,
        has_one = user,
        close = user
    )]
    pub stake_account: Account<'info, StakeAccount>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"vault", mint.key().as_ref()],
        bump = vault_bump
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [b"reward_mint", collection_account.key().as_ref()],
        bump = collection_account.reward_mint_bump
    )]
    pub reward_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = reward_mint,
        associated_token::authority = user
    )]
    pub user_reward_account: Account<'info, TokenAccount>,

    pub vault_bump: u8,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

// Data structures
#[account]
pub struct CollectionAccount {
    pub authority: Pubkey,
    pub max_supply: u64,
    pub current_supply: u64,
    pub reward_mint: Pubkey,
    pub reward_mint_bump: u8,
    pub total_staked: u64,
    pub rewards_per_token_stored: u64,
    pub last_update_time: i64,
    pub bump: u8,
}

impl CollectionAccount {
    const LEN: usize = 8 + 32 + 8 + 8 + 32 + 1 + 8 + 8 + 8 + 1;
}

#[account]
pub struct StakeAccount {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub stake_time: i64,
    pub rewards_per_token_paid: u64,
    pub pending_rewards: u64,
    pub bump: u8,
}

impl StakeAccount {
    const LEN: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

// Error codes
#[error_code]
pub enum StakingError {
    #[msg("Maximum supply reached")]
    MaxSupplyReached,
    #[msg("NFT not owned by user")]
    NotOwner,
    #[msg("NFT not staked")]
    NotStaked,
}