use crate::{errors::EscrowError, state::Escrow};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{
        close_account, transfer_checked, CloseAccount, Mint, TokenAccount, TokenInterface,
        TransferChecked,
    },
};

#[derive(Accounts)]
pub struct Refund<'info> {
    // 签名账户, 即创建托管的账户
    #[account(mut)]
    pub maker: Signer<'info>,

    // 托管账户的数据账户, 此时不需要 init, 因为这个账户在 make 阶段已经初始化了
    #[account(
        mut,
        close = maker,
        seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()],
        bump = escrow.bump,
        has_one = maker @ EscrowError::InvalidMaker,
        has_one = mint_a @ EscrowError::InvalidMintA
    )]
    pub escrow: Account<'info, Escrow>,

    // Token A 的 mint 账户
    #[account(mint::token_program = token_program)]
    pub mint_a: InterfaceAccount<'info, Mint>,

    // 托管资金 ATA 账户
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = escrow,
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    // 创建者所存入的 Token A 的 ATA 账户
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = maker,
        associated_token::token_program = token_program
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    // 账户所需要的程序
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Refund>) -> Result<()> {
    let vault = &ctx.accounts.vault;
    let maker = &ctx.accounts.maker;
    let escrow = &ctx.accounts.escrow;
    let mint_a = &ctx.accounts.mint_a;
    let escrow_seed_le_bytes = escrow.seed.to_le_bytes();
    let amount = vault.amount;
    let decimals = mint_a.decimals;

    // 将 vault 的 token A 转账给 maker
    let signer_seeds: &[&[&[u8]]] = &[&[
        b"escrow",
        maker.to_account_info().key.as_ref(),
        escrow_seed_le_bytes.as_ref(),
        &[escrow.bump],
    ]];

    // 只有托管账户中的 token A 大于 0 时, 才需要转账
    if amount > 0 {
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    authority: ctx.accounts.escrow.to_account_info(),
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.maker_ata_a.to_account_info(),
                    mint: ctx.accounts.mint_a.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
            decimals,
        )?;
    };

    // 关闭 vault 账户
    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
            destination: ctx.accounts.maker.to_account_info(),
        },
        signer_seeds,
    ))?;

    Ok(())
}
