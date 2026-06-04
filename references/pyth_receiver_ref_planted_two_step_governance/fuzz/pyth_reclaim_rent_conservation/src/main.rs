// invariant_reclaim_rent_returns_to_write_authority
//
// cf-invariants-pyth Phase-3.5 fixture — reclaim_rent_conservation class.
// Target: Crucible v0.2.0 (asymmetric-research/crucible).
// Source: Heuristic (suggester v0.2.0). AI suggestion captured separately.
//
// `reclaim_rent` closes a `PriceUpdateV2` account and returns its lamports.
// Conservation: rent MUST return only to the recorded `write_authority`. An
// attacker signing `reclaim_rent` with a fresh keypair must be rejected.
//
// Setup: pre-bake a `PriceUpdateV2` account via Crucible's `write_anchor_account`
// — this sidesteps the `post_update` / `post_update_atomic` paths (which would
// require a guardian-signing VAA-crafter, Phase-3's honest-bound surprise).
// The pre-baked PriceUpdateV2 has `write_authority = write_authority.pubkey()`
// and a known lamport balance.
//
// Attacker arm: `action_attack_reclaim_rent` mints a fresh `attacker` keypair,
// funds it, and tries to call `reclaim_rent` as `payer = attacker`. A clean
// program rejects with `WrongWriteAuthority`; a planted program (constraint
// dropped from the ReclaimRent Accounts struct) accepts and drains the
// PriceUpdateV2 rent into the attacker. The sticky `unauthorized_reclaim_observed`
// flag captures the conservation violation.

#![allow(unused_imports)]

use crucible_fuzzer::anchor_lang::system_program;
use crucible_fuzzer::*;
use ::pyth_solana_receiver::*;
use ::pyth_solana_receiver_sdk::config::Config;
use ::pyth_solana_receiver_sdk::pda::CONFIG_SEED;
use ::pyth_solana_receiver_sdk::price_update::{PriceUpdateV2, VerificationLevel};
use ::pythnet_sdk::messages::PriceFeedMessage;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::rc::Rc;

const INITIAL_BALANCE: u64 = 10_000_000_000;
/// Lamports seeded into the PriceUpdateV2 PDA — large enough that draining
/// it is conspicuous, small enough that overflow is impossible.
const PRICE_UPDATE_LAMPORTS: u64 = 5_000_000_000;

#[derive(Clone)]
struct PythReclaimRentConservationFixture {
    ctx: TestContext,
    program_id: Pubkey,
    initializer: Rc<Keypair>,
    config_pda: Pubkey,
    /// The authorized write_authority that legitimately owns the PriceUpdateV2.
    write_authority: Rc<Keypair>,
    /// The PriceUpdateV2 account pre-baked via `write_anchor_account`.
    price_update_pubkey: Pubkey,
    /// Sticky flag — set to true on any successful attacker reclaim. The
    /// invariant asserts this stays false for the lifetime of the run.
    unauthorized_reclaim_observed: bool,
}

#[fuzz_fixture]
impl PythReclaimRentConservationFixture {
    pub fn setup() -> Self {
        let mut ctx = TestContext::new();
        let program_id = Pubkey::new_from_array(ID.to_bytes());
        ctx.add_program(
            &program_id,
            "../../target/deploy/pyth_solana_receiver.so",
        )
        .unwrap();

        // Funded initializer + governance keypairs. The Config seeding mirrors
        // the two_step_governance fixture (no shared trait — fuzz fixtures are
        // intentionally self-contained per the cf-invariants-jito convention).
        let initializer = Rc::new(Keypair::new());
        ctx.create_account()
            .pubkey(initializer.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        let authority = Keypair::new();

        let (config_pda, _config_bump) =
            Pubkey::find_program_address(&[CONFIG_SEED.as_ref()], &program_id);

        let initial_config = Config {
            governance_authority: authority.pubkey(),
            target_governance_authority: None,
            wormhole: Keypair::new().pubkey(),
            valid_data_sources: vec![],
            single_update_fee_in_lamports: 0,
            minimum_signatures: 5,
        };
        ctx.program(program_id)
            .call(instruction::Initialize { initial_config })
            .accounts(accounts::Initialize {
                payer: initializer.pubkey(),
                config: config_pda,
                system_program: system_program::ID,
            })
            .signers(&[&*initializer])
            .send()
            .unwrap();

        // The legitimate write_authority. Funded so a sibling arm could call
        // reclaim_rent if we wanted to exercise the happy path; the
        // attacker-probe is the invariant-relevant probe.
        let write_authority = Rc::new(Keypair::new());
        ctx.create_account()
            .pubkey(write_authority.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        // Pre-bake the PriceUpdateV2 account. The fixture writes the
        // discriminator + serialized PriceUpdateV2 via `write_anchor_account`
        // so we don't have to drive post_update_atomic (which would require a
        // guardian-signing VAA-crafter — see Phase-3 outbox report §2.2 for why
        // that path was retired from this harness).
        let price_update_pubkey = Keypair::new().pubkey();
        let price_update_state = PriceUpdateV2 {
            write_authority: write_authority.pubkey(),
            verification_level: VerificationLevel::Full,
            price_message: PriceFeedMessage {
                feed_id: [0u8; 32],
                price: 0,
                conf: 0,
                exponent: 0,
                publish_time: 0,
                prev_publish_time: 0,
                ema_price: 0,
                ema_conf: 0,
            },
            posted_slot: 0,
        };
        ctx.create_account()
            .pubkey(price_update_pubkey)
            .lamports(PRICE_UPDATE_LAMPORTS)
            .owner(program_id)
            .size(PriceUpdateV2::LEN)
            .create()
            .unwrap();
        ctx.write_anchor_account(&price_update_pubkey, &price_update_state)
            .unwrap();

        Self {
            ctx,
            program_id,
            initializer,
            config_pda,
            write_authority,
            price_update_pubkey,
            unauthorized_reclaim_observed: false,
        }
    }

    /// Attacker arm — probes `reclaim_rent` with a freshly-minted attacker
    /// keypair. A clean program rejects with `WrongWriteAuthority` (the
    /// `ReclaimRent` Accounts struct constraint); the planted variant drops
    /// the constraint and accepts the call, draining the PriceUpdateV2 into
    /// the attacker. The sticky flag captures any success.
    pub fn action_attack_reclaim_rent(&mut self) -> bool {
        let attacker = Keypair::new();
        // Fund the attacker so a missing signer-check is the ONLY reason
        // the call could succeed (rent / fee won't be the blocker).
        let _ = self.ctx
            .create_account()
            .pubkey(attacker.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create();

        let attempted = self.ctx
            .program(self.program_id)
            .call(instruction::ReclaimRent {})
            .accounts(accounts::ReclaimRent {
                payer: attacker.pubkey(),
                price_update_account: self.price_update_pubkey,
            })
            .signers(&[&attacker])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);

        if attempted {
            self.unauthorized_reclaim_observed = true;
        }
        true
    }
}

// reclaim_rent_conservation invariant.
//
// If the program ever accepted a `reclaim_rent` call signed by anyone other
// than the recorded `PriceUpdateV2.write_authority`, the sticky flag is `true`
// and this assertion fails. The clean program rejects via the
// `WrongWriteAuthority` constraint; the planted variant drops the constraint.
#[invariant_test]
fn invariant_reclaim_rent_returns_to_write_authority(
    fixture: &mut PythReclaimRentConservationFixture,
) {
    fuzz_assert_eq!(
        fixture.unauthorized_reclaim_observed, false,
        "unauthorized reclaim_rent succeeded against PriceUpdateV2 {}",
        fixture.price_update_pubkey
    );
}
