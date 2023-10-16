use anchor_lang::{
    prelude::*,
    AnchorSerialize, AnchorDeserialize,
    solana_program::{
        system_instruction
    }
};
use anchor_spl::token::{self, Token, TokenAccount, Mint, SetAuthority};
use spl_token::instruction::AuthorityType;
use metaplex_token_metadata::{
    instruction::{mint_new_edition_from_master_edition_via_token},
    state::MasterEditionV2
};

declare_id!("38sac6kMWbM8gxNeTAAFTsHLnDvVmB7q85vu6DStGRB9");

#[program]
pub mod print_maker{
    use anchor_lang::solana_program::program::{invoke, invoke_signed};

    use super::*;

    pub fn init_pool(
        ctx : Context<InitPool>,
        _bump : u8,
        _rarity : Vec<Rarity>,
        _price : u64,
        ) -> Result<()> {
        msg!("+ init_pool");
        let pool = &mut ctx.accounts.pool;
        let mut total : u8 = 0;
        for r in _rarity.iter(){
            total += r.supply;
        }
        if total != 100{
            return Err(PoolError::InvalidTotalSupply.into());
        }
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info().clone(),
            SetAuthority{
                current_authority : ctx.accounts.owner.to_account_info().clone(),
                account_or_mint : ctx.accounts.nft_account.to_account_info().clone()
            }
        );
        token::set_authority(cpi_ctx, AuthorityType::AccountOwner, Some(pool.key()))?;
        pool.owner = ctx.accounts.owner.key();
        pool.nft_mint = ctx.accounts.nft_mint.key();
        pool.nft_account = ctx.accounts.nft_account.key();
        pool.rand = *ctx.accounts.rand.key;
        pool.price = _price;
        pool.rarity = _rarity;
        pool.available = true;
        pool.bump = _bump;
        Ok(())
    }

    pub fn redeem(
        ctx : Context<Redeem>,
        ) -> Result<()> {
        msg!("+ redeem");
        let pool = &mut ctx.accounts.pool;
        let pool_signer_seeds = &[pool.rand.as_ref(),&[pool.bump]];
        let signer = &[&pool_signer_seeds[..]];
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info().clone(),
            SetAuthority{
                current_authority : pool.to_account_info().clone(),
                account_or_mint : ctx.accounts.nft_account.to_account_info().clone()
            },
            signer
        );
        token::set_authority(cpi_ctx, AuthorityType::AccountOwner, Some(pool.owner))?;
        pool.available = false;
        Ok(())
    }

    pub fn mint_one_print<'a, 'b, 'c, 'info>(
        ctx : Context<'_, '_, '_, 'info, MintOneToken<'info>>
        )->Result<()>{
        let pool = &mut ctx.accounts.pool;
        let treasury_wallets = &ctx.remaining_accounts;
        if ctx.accounts.owner.lamports() < pool.price{
            return Err(PoolError::NotEnoughSol.into());
        }
        if pool.rarity.len() != treasury_wallets.len(){
            return Err(PoolError::InvalidTreasuryWallets.into());
        }
        for (i, r) in pool.rarity.iter().enumerate(){
            let treasury = &treasury_wallets[i];
            if r.address != *treasury.key{
                return Err(PoolError::InvalidTreasuryWallets.into());
            }
            invoke(
                &system_instruction::transfer(
                    &ctx.accounts.owner.key(),
                    &r.address,
                    pool.price * r.supply as u64 / 100
                ),
                &[
                    ctx.accounts.owner.to_account_info().clone(),
                    treasury.clone(),
                    ctx.accounts.system_program.to_account_info().clone()
                ] 
            )?;
        }
        let pool_signer_seeds = &[pool.rand.as_ref(),&[pool.bump]];
        let edition =  MasterEditionV2::from_account_info(&ctx.accounts.nft_master_edition)?;

        invoke_signed(
            &mint_new_edition_from_master_edition_via_token(
                *ctx.accounts.metadata_program.key,
                *ctx.accounts.new_metadata.key,
                *ctx.accounts.new_edition.key,
                *ctx.accounts.nft_master_edition.key,
                ctx.accounts.new_mint.key(),
                ctx.accounts.owner.key(),
                ctx.accounts.owner.key(),
                pool.key(),
                pool.nft_account,
                pool.key(),
                *ctx.accounts.nft_metadata.key,
                ctx.accounts.nft_mint.key(),
                edition.supply+1
            ),
            &[
                ctx.accounts.metadata_program.clone(),
                ctx.accounts.new_metadata.clone(),
                ctx.accounts.new_edition.clone(),
                ctx.accounts.nft_master_edition.clone(),
                ctx.accounts.new_mint.to_account_info().clone(),
                ctx.accounts.owner.to_account_info().clone(),
                pool.to_account_info().clone(),
                ctx.accounts.nft_account.to_account_info().clone(),
                ctx.accounts.nft_metadata.clone(),
                ctx.accounts.nft_mint.to_account_info().clone(),
                ctx.accounts.edition_mark_pda.clone(),
                ctx.accounts.rent.to_account_info().clone(),
            ],
            &[pool_signer_seeds]
        )?;
        Ok(())
    }
}

#[instruction(_bump : u8, _rarity : Vec<Rarity>)]
#[derive(Accounts)]
pub struct InitPool<'info>{
    #[account(mut)]
    owner : Signer<'info>,

    #[account(
        init,
        seeds=[(*rand.key).as_ref()],
        bump,
        payer=owner,
        space=8 + POOL_SIZE + 4 + RARITY_SIZE * _rarity.len())]
    pool : Account<'info, Pool>,

    /// CHECK: Rand Address
    rand : AccountInfo<'info>,

    nft_mint : Account<'info, Mint>,

    #[account(mut, 
        constraint = nft_account.owner==owner.key()
            && nft_account.mint==nft_mint.key())]
    nft_account : Account<'info, TokenAccount>,

    token_program : Program<'info, Token>,

    system_program : Program<'info, System>,
}

#[derive(Accounts)]
pub struct Redeem<'info>{
    #[account(mut)]
    owner : Signer<'info>,

    #[account(mut,
        constraint = pool.owner==owner.key()
            && pool.nft_account==nft_account.key())]
    pool : Account<'info, Pool>,

    #[account(mut,
        constraint = nft_account.owner==pool.key())]
    nft_account : Account<'info, TokenAccount>,

    token_program : Program<'info, Token>
}

#[derive(Accounts)]
pub struct MintOneToken<'info>{
    #[account(mut)]
    owner : Signer<'info>,

    #[account(mut,
        has_one=nft_account,
        constraint=pool.available==true)]
    pool : Account<'info, Pool>,

    nft_mint : Account<'info, Mint>,

    #[account(constraint=nft_account.mint==nft_mint.key())]
    nft_account : Account<'info, TokenAccount>,

    /// CHECK: Metadata
    nft_metadata : AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Master Edition
    nft_master_edition : AccountInfo<'info>,

    #[account(mut,
        constraint=new_mint.decimals==0 && new_mint.supply==1)]
    new_mint : Account<'info, Mint>,

    #[account(mut)]
    /// CHECK: new Metadata
    new_metadata : AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: new Edition
    new_edition : AccountInfo<'info>,

    #[account(mut)]
    /// CHECK
    edition_mark_pda : AccountInfo<'info>,

    token_program : Program<'info, Token>,

    /// CHECK
    metadata_program : AccountInfo<'info>,

    system_program : Program<'info, System>,

    rent : Sysvar<'info, Rent>
}

const RARITY_SIZE : usize = 32+1;
const POOL_SIZE : usize = 32+32+32+32+8+1+1;

#[account]
pub struct Pool{
    pub owner : Pubkey,
    pub nft_mint : Pubkey,
    pub nft_account : Pubkey,
    pub rand : Pubkey,
    pub price : u64,
    pub rarity : Vec<Rarity>,
    pub available : bool,
    pub bump : u8
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone, Copy)]
pub struct Rarity{
    pub address : Pubkey,
    pub supply : u8,
}

#[error_code]
pub enum PoolError{
    #[msg("Invalid metadata")]
    InvalidMetadata,

    #[msg("Invalid total supply")]
    InvalidTotalSupply,

    #[msg("Not enough sol")]
    NotEnoughSol,

    #[msg("Invalid treasury wallets")]
    InvalidTreasuryWallets,
}
