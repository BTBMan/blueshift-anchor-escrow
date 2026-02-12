use crate::{errors::EscrowError, state::Escrow};
use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

// 定义 make 所需的账户列表
#[derive(Accounts)]
#[instruction(seed: u64)] // 用来获取指令中的参数, 这里只获取了 seed 传参
pub struct Make<'info> {
    // 签名账户, 即创建托管的账户
    #[account(mut)]
    pub maker: Signer<'info>,

    // 初始化托管 PDA 数据账户, 主要用来存放所需要的数据
    #[account(
        init,
        payer = maker, // 指定创建账户所花费用的支付者
        space = Escrow::INIT_SPACE + Escrow::DISCRIMINATOR.len(), // 默认的 8 个字节的判别符空间 8 替换为 Escrow::DISCRIMINATOR.len()
        seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()], // 通过 maker 和自定义传入的 seed 来生成 pda
        bump,
    )]
    pub escrow: Account<'info, Escrow>,

    // 存入的 Token A 的 mint 账户
    #[account(
        mint::token_program = token_program // 约束 mint_a 的 token_program 必须是 token_program(SPL Token Program)
    )]
    pub mint_a: InterfaceAccount<'info, Mint>,

    // 换取的 Token B 的 mint 账户
    #[account(
        mint::token_program = token_program
    )]
    pub mint_b: InterfaceAccount<'info, Mint>,

    // 创建者所想换取的 Token A 的 ATA 账户
    #[account(
        mut,
        associated_token::mint = mint_a, // 约束 ATA 账户是和 mint_a 绑定的,
        associated_token::authority = maker, // 约束这是创建者的 ATA 账户
        associated_token::token_program = token_program // 约束 associated token program 创建账户应该使用那个 token program 来管理这个账户
    )]
    pub maker_ata_a: InterfaceAccount<'info, TokenAccount>,

    // 创建和初始化资金托管 ATA 账户, 关联 mint_a 账户, 用来存取 token_a
    // 不需要 init, 因为 ATA 账户的大小是固定的(固定的几个字段, 如: amount, owner 等), Associated Token Program 会自动分配大小
    #[account(
        init,
        payer = maker,
        associated_token::mint = mint_a,
        associated_token::authority = escrow, // 约束这是 escrow 的 ATA 账户
        associated_token::token_program = token_program
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    // Programs
    pub associated_token_program: Program<'info, AssociatedToken>, // ATA 程序(因为需要定义 ATA 账户, 所以必须显示定义 AssociatedTokenAccount 程序)
    pub token_program: Interface<'info, TokenInterface>,           // SPL Token 程序
    pub system_program: Program<'info, System>,                    // 系统程序
}

impl<'info> Make<'info> {
    /// # Create the Escrow
    fn populate_escrow(&mut self, seed: u64, amount: u64, bump: u8) -> Result<()> {
        self.escrow.set_inner(Escrow {
            seed,
            maker: self.maker.key(),
            mint_a: self.mint_a.key(),
            mint_b: self.mint_b.key(),
            receive: amount,
            bump,
        });

        Ok(())
    }

    /// # Deposit the tokens
    fn deposit_tokens(&self, amount: u64) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.maker_ata_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    to: self.vault.to_account_info(),
                    authority: self.maker.to_account_info(),
                },
            ),
            amount,
            self.mint_a.decimals,
        )?;

        Ok(())
    }
}

pub fn handler(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
    // Validate the amount
    require_gt!(receive, 0, EscrowError::InvalidAmount);
    require_gt!(amount, 0, EscrowError::InvalidAmount);

    // Save the Escrow Data
    ctx.accounts
        .populate_escrow(seed, receive, ctx.bumps.escrow)?;

    // Deposit Tokens
    ctx.accounts.deposit_tokens(amount)?;

    Ok(())
}
