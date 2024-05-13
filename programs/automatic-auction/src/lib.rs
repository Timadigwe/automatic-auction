use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    clock::Clock,
    instruction::Instruction,
    native_token::LAMPORTS_PER_SOL,
    program::{invoke, invoke_signed},
    system_program,
    sysvar::Sysvar,
};
use anchor_lang::InstructionData;
use anchor_lang::{AccountDeserialize, AnchorDeserialize};
use anchor_spl::token::{self, TokenAccount, Transfer};
use clockwork_sdk::state::{Thread, ThreadAccount};

declare_id!("HMmKpGJBCdmCp9XMv4YVrnYppr1c5Wi36ptVTXGfs75y");

#[program]
pub mod automatic_auction {
    use super::*;
    pub fn create_auction(
        ctx: Context<CreateAuction>,
        thread_id: Vec<u8>,
        start_price: u64,
        end_time: i64,
    ) -> Result<()> {
        //Get the accounts
        let system_program = &ctx.accounts.system_program;
        let clockwork_program = &ctx.accounts.clockwork_program;
        let signer = &ctx.accounts.signer;
        let thread = &ctx.accounts.thread;
        let thread_authority = &ctx.accounts.thread_authority;
        let auction = &mut ctx.accounts.auction;
        auction.ongoing = true;
        auction.seller = *ctx.accounts.seller.key();
        auction.price = start_price;
        auction.end_time = end_time;

        //Prepare an instruction to automate.

        Ok(())
    }

    pub fn bid(ctx: Context<Bid>, price: u64) -> Result<()> {
        let auction = &mut ctx.accounts.auction;

        let current_time = Clock::get()?.unix_timestamp;

        //Ensure that the bid has not ended
        require_eq!(auction.ongoing, true, AuctionErr::AuctionEnded);
        require_gt!(auction.end_time, current_time, AuctionErr::AuctionEnded);
        require_gt!(price, auction.price, AuctionErr::BidPirceTooLow);

        // if a previous bidder exists refund the money
        if auction.bidder != Pubkey::default() {
            let signer = ctx.accounts.signer.key;
            let bump = ctx.bumps.auction;
            let seeds = &[b"auction_owner_pda".as_ref(), signer.as_ref(), &[bump]];
            let signer = &[&seeds[..]];

            let transfer_instruction = Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.bidder.to_account_info(),
                authority: ctx.accounts.auction.to_account_info(),
            };

            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                transfer_instruction,
                signer,
            );

            anchor_spl::token::transfer(cpi_ctx, auction.price.clone())?;
        }

        //transfer bid price to vault
        let transfer_instruction = Transfer {
            from: ctx.accounts.bidder.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.signer.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
        );

        anchor_spl::token::transfer(cpi_ctx, price)?;

        // update auction info
        auction.bidder = *ctx.accounts.bidder.key();
        auction.price = price;

        Ok(())
    }

    pub fn close_auction(ctx: Context<CloseAuction>) -> Result<()> {
        let auction = &mut ctx.accounts.auction;
        let signer = ctx.accounts.signer.key;
        let bump = ctx.bumps.auction;
        let seeds = &[b"auction_owner_pda".as_ref(), signer.as_ref(), &[bump]];
        let signer = &[&seeds[..]];

        let transfer_instruction = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.bid_winner.to_account_info(),
            authority: ctx.accounts.auction.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
            signer,
        );

        anchor_spl::token::transfer(cpi_ctx, auction.price.clone())?;
        auction.ongoing = false;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(thread_id: Vec < u8 >)]
pub struct CreateAuction<'info> {
    #[account(
    init_if_needed,
    payer = signer,
    seeds=[b"auction_owner_pda", signer.key().as_ref()],
    bump,
    space = 8 + 1 + 32 + 32  + 8 + 8
  )]
    auction: Account<'info, Auction>,

    #[account(
        init_if_needed,
        payer = signer,
        seeds=[b"vault", mint_of_token_being_sent.key().as_ref(),signer.key().as_ref()],
        token::mint = mint_of_token_being_sent,
        token::authority = auction_owner_pda,
        bump
    )]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    seller: Account<'info, TokenAccount>,

    mint_of_token_being_sent: Account<'info, Mint>,

    #[account(mut)]
    signer: Signer<'info>,

    /// Address to assign to the newly created thread.
    #[account(mut, address = Thread::pubkey(thread_authority.key(), thread_id))]
    pub thread: SystemAccount<'info>,

    /// The pda that will own and manage the thread.
    #[account(seeds = [b"authority"], bump)]
    pub thread_authority: SystemAccount<'info>,

    /// The Clockwork thread program.
    #[account(address = clockwork_sdk::ID)]
    pub clockwork_program: Program<'info, clockwork_sdk::ThreadProgram>,

    #[account(address = system_program::ID)]
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Bid<'info> {
    #[account(
     mut,
    seeds=[b"auction_owner_pda", signer.key().as_ref()],
    bump,
  )]
    auction: Account<'info, Auction>,

    #[account(
        mut,
        seeds=[b"vault", mint_of_token_being_sent.key().as_ref(),signer.key().as_ref()],
        bump
    )]
    vault: Account<'info, TokenAccount>,

    #[account(mut)]
    bidder: Account<'info, TokenAccount>,

    mint_of_token_being_sent: Account<'info, Mint>,

    #[account(mut)]
    signer: Signer<'info>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CloseAuction<'info> {
    #[account(
     mut,
    seeds=[b"auction_owner_pda", signer.key().as_ref()],
    bump,
  )]
    auction: Account<'info, Auction>,

    #[account(
        mut,
        seeds=[b"vault", mint_of_token_being_sent.key().as_ref(),signer.key().as_ref()],
        bump
    )]
    vault: Account<'info, TokenAccount>,

    /// Verify that only this thread can execute the Increment Instruction
    #[account(signer, constraint = thread.authority.eq(&thread_authority.key()))]
    pub thread: Account<'info, Thread>,

    /// The Thread Admin
    /// The authority that was used as a seed to derive the thread address
    /// `thread_authority` should equal `thread.thread_authority`
    #[account(seeds = [b"authority"], bump)]
    pub thread_authority: SystemAccount<'info>,

    #[account(mut)]
    bid_winner: Account<'info, TokenAccount>,

    mint_of_token_being_sent: Account<'info, Mint>,

    #[account(mut)]
    signer: Signer<'info>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[account]
pub struct Auction {
    ongoing: bool,
    seller: Pubkey,
    bidder: Pubkey,
    price: u64,
    end_time: i64,
}

#[error_code]
pub enum AuctionErr {
    #[msg("your bid price is too low")]
    BidPirceTooLow,
    #[msg("Auction has already ended")]
    AuctionEnded,
}

