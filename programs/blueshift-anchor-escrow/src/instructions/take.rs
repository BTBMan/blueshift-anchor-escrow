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
pub struct Take<'info> {
    // 签名账户, 是取走托管资金 Token A 的账户
    #[account(mut)]
    pub taker: Signer<'info>,

    // 托管账户的创建者, 须要把 Token B 转账给这个账户
    #[account(mut)]
    pub maker: SystemAccount<'info>,

    // 托管账户的数据账户, 此时不需要 init, 因为这个账户在 make 阶段已经初始化了
    #[account(
      mut,
      close = maker, // 关闭数据账户, 租金归 maker 所有
      seeds = [b"escrow", maker.key().as_ref(), escrow.seed.to_le_bytes().as_ref()], // 数据账户的种子
      bump = escrow.bump, // 数据账户的 bump 值
      has_one = maker @ EscrowError::InvalidMaker, // 验证数据账户的 maker 是否是 maker
      has_one = mint_a @ EscrowError::InvalidMintA, // 验证数据账户的 mint_a 是否是 mint_a
      has_one = mint_b @ EscrowError::InvalidMintB, // 验证数据账户的 mint_b 是否是 mint_b
  )]
    pub escrow: Box<Account<'info, Escrow>>, // 使用 Box 减少 stack 的大小

    // Token A 和 Token B 的 mint 账户
    pub mint_a: Box<InterfaceAccount<'info, Mint>>,
    pub mint_b: Box<InterfaceAccount<'info, Mint>>,

    // 托管资金 ATA 账户
    #[account(
      mut,
      associated_token::mint = mint_a,
      associated_token::authority = escrow,
      associated_token::token_program = token_program
  )]
    pub vault: Box<InterfaceAccount<'info, TokenAccount>>,

    // 取款者的 Token A 的 ATA 账户, 用来接收 Token A, init_if_needed 表示如果不存在则创建(有可能 taker 没有这个账户)
    #[account(
      init_if_needed,
      payer = taker,
      associated_token::mint = mint_a,
      associated_token::authority = taker,
      associated_token::token_program = token_program
  )]
    pub taker_ata_a: Box<InterfaceAccount<'info, TokenAccount>>,

    // 取款者的 Token B 的 ATA 账户, 用来把 Token B 转账给 maker
    #[account(
      mut,
      associated_token::mint = mint_b,
      associated_token::authority = taker,
      associated_token::token_program = token_program
  )]
    pub taker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    // 托管账户创建者的 Token B 的 ATA 账户, 用来接收所希望换取的 Token B
    #[account(
      init_if_needed,
      payer = taker,
      associated_token::mint = mint_b,
      associated_token::authority = maker,
      associated_token::token_program = token_program
  )]
    pub maker_ata_b: Box<InterfaceAccount<'info, TokenAccount>>,

    // 账户所需要的程序
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Take<'info> {
    // 把 Token B 转账给 maker
    fn transfer_to_maker(&mut self) -> Result<()> {
        transfer_checked(
            CpiContext::new(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.taker_ata_b.to_account_info(),
                    to: self.maker_ata_b.to_account_info(),
                    mint: self.mint_b.to_account_info(),
                    authority: self.taker.to_account_info(),
                },
            ),
            self.escrow.receive,
            self.mint_b.decimals,
        )?;

        Ok(())
    }

    // 从 vault 中取出 Token A 转账给 taker 并关闭 vault
    fn withdraw_and_close_vault(&mut self) -> Result<()> {
        // 由于是从 vault PDA 账户中转账, 因此需要提供 PDA 的签名 seeds
        let signer_seeds: [&[&[u8]]; 1] = [&[
            b"escrow",
            self.maker.to_account_info().key.as_ref(),
            &self.escrow.seed.to_le_bytes()[..],
            &[self.escrow.bump],
        ]];

        // 把 Token A 转账给 taker
        transfer_checked(
            CpiContext::new_with_signer(
                self.token_program.to_account_info(),
                TransferChecked {
                    from: self.vault.to_account_info(),
                    to: self.taker_ata_a.to_account_info(),
                    mint: self.mint_a.to_account_info(),
                    authority: self.escrow.to_account_info(),
                },
                &signer_seeds,
            ),
            self.vault.amount,
            self.mint_a.decimals,
        )?;

        // 关闭 vault 账户
        close_account(CpiContext::new_with_signer(
            self.token_program.to_account_info(),
            CloseAccount {
                account: self.vault.to_account_info(),
                authority: self.escrow.to_account_info(),
                destination: self.maker.to_account_info(), // 关闭账户后的租金去向, 还给 maker
            },
            &signer_seeds,
        ))?;

        Ok(())
    }
}

pub fn handler(ctx: Context<Take>) -> Result<()> {
    // 转账 Token B 给 maker
    ctx.accounts.transfer_to_maker()?;

    // 从 vault 中取出 Token A 转账给 taker 并关闭 vault
    ctx.accounts.withdraw_and_close_vault()?;

    // 指令执行完毕后 anchor 自动关闭 escrow 数据账户

    Ok(())
}
