use anchor_lang::prelude::*;

#[derive(InitSpace)] // 不需要手动计算空间大小(租金)
#[account(discriminator = 1)] // 用自定义的标识符 1 代替默认账户名称哈希后的前 8 个字节
pub struct Escrow {
    // 随机数, 用于生成不同的 Escrow 账户
    pub seed: u64,
    // 托管账户的创建者
    pub maker: Pubkey,
    // 存入的 token A 的 mint 账户地址
    pub mint_a: Pubkey,
    // 换取的 token B 的 mint 账户地址
    pub mint_b: Pubkey,
    // 创建者希望收到的 Token B 的数量
    pub receive: u64,
    // 缓存的 bump 值, 防止动态派生所消耗的计算资源
    pub bump: u8,
}
