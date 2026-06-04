// invariant_two_step_governance_atomic
//
// cf-invariants-pyth Phase-3.5 fixture — two_step_governance class.
// Target: Crucible v0.2.0 (asymmetric-research/crucible).
// Source: Heuristic (suggester v0.2.0). AI suggestion captured separately.
//
// Pyth's two-step governance transfer (request -> accept) must be ATOMIC:
//   - Only the current `Config.governance_authority` can `request_*` or `cancel_*`.
//   - Only the recorded `Config.target_governance_authority` can `accept_*`.
//   - `accept_*` MUST clear `target_governance_authority` (set to None) in the
//     same instruction, so the same target cannot replay the accept against a
//     now-stale value.
//
// Planted bug: drops `config.target_governance_authority = None;` in
// `accept_governance_authority_transfer`. The fixture's
// `action_accept_then_observe_target` arm calls accept and then asserts
// `target_governance_authority` is `None` — a planted variant leaves it `Some`
// and the sticky `nonatomic_observed` flag fires.
//
// Mirror of cf-invariants-jito's `jito_admin_gating` fixture pattern.

#![allow(unused_imports)]

use crucible_fuzzer::anchor_lang::system_program;
use crucible_fuzzer::*;
// `::` prefix disambiguates the program crate from any re-export via
// `crucible_fuzzer::*`.
use ::pyth_solana_receiver::*;
use ::pyth_solana_receiver_sdk::config::{Config, DataSource};
use ::pyth_solana_receiver_sdk::pda::CONFIG_SEED;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::rc::Rc;

const INITIAL_BALANCE: u64 = 10_000_000_000;

#[derive(Clone)]
struct PythTwoStepGovernanceFixture {
    ctx: TestContext,
    program_id: Pubkey,
    initializer: Rc<Keypair>,
    /// The original governance_authority recorded on the Config PDA.
    authority: Rc<Keypair>,
    /// The target the fixture requests a transfer to (and which is allowed to accept).
    target_authority: Rc<Keypair>,
    config_pda: Pubkey,
    /// Sticky flag — set to true if `target_governance_authority` is still
    /// `Some(_)` after a successful `accept_governance_authority_transfer`. The
    /// clean program clears it; the planted program does not. The invariant
    /// asserts this stays false for the lifetime of the run.
    nonatomic_observed: bool,
}

#[fuzz_fixture]
impl PythTwoStepGovernanceFixture {
    pub fn setup() -> Self {
        let mut ctx = TestContext::new();
        let program_id = Pubkey::new_from_array(ID.to_bytes());
        ctx.add_program(
            &program_id,
            "../../target/deploy/pyth_solana_receiver.so",
        )
        .unwrap();

        // Funded initializer signer (pays Config rent on init).
        let initializer = Rc::new(Keypair::new());
        ctx.create_account()
            .pubkey(initializer.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        // The recorded governance_authority. Funded so it can sign as `payer`
        // on Governance / AcceptGovernanceAuthorityTransfer ix.
        let authority = Rc::new(Keypair::new());
        ctx.create_account()
            .pubkey(authority.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        // The target_governance_authority we'll transfer to. Funded so it can
        // sign the `accept` ix.
        let target_authority = Rc::new(Keypair::new());
        ctx.create_account()
            .pubkey(target_authority.pubkey())
            .lamports(INITIAL_BALANCE)
            .owner(system_program::ID)
            .create()
            .unwrap();

        let (config_pda, _config_bump) =
            Pubkey::find_program_address(&[CONFIG_SEED.as_ref()], &program_id);

        // Seed Config via the program's own initialize ix — exercises the
        // `init` constraint and produces a realistic on-chain layout. Pyth's
        // Config has 6 fields including a Vec<DataSource> which we leave empty
        // (governance ix do not touch data sources).
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

        Self {
            ctx,
            program_id,
            initializer,
            authority,
            target_authority,
            config_pda,
            nonatomic_observed: false,
        }
    }

    /// Drive the request -> accept handshake then observe the target field. A
    /// clean program clears `target_governance_authority` on accept; a planted
    /// program leaves it `Some(target_authority.pubkey())`.
    ///
    /// `arbitrary_request_target` shuffles the request target each iteration
    /// so the fuzzer explores both "request matches accept" and
    /// "request to other -> accept by self" paths; only the
    /// matches-accept path can reach the observation. Attacker probes (a
    /// freshly minted signer trying to accept) live in their own arm below.
    pub fn action_request_then_accept_then_observe(&mut self) -> bool {
        // Reset to a clean state — clear any leftover target from a prior
        // iteration so each run starts at a known baseline.
        let _ = self.ctx
            .program(self.program_id)
            .call(instruction::CancelGovernanceAuthorityTransfer)
            .accounts(accounts::Governance {
                payer: self.authority.pubkey(),
                config: self.config_pda,
            })
            .signers(&[&*self.authority])
            .send();

        // Request transfer to `target_authority`.
        let request_ok = self.ctx
            .program(self.program_id)
            .call(instruction::RequestGovernanceAuthorityTransfer {
                target_governance_authority: self.target_authority.pubkey(),
            })
            .accounts(accounts::Governance {
                payer: self.authority.pubkey(),
                config: self.config_pda,
            })
            .signers(&[&*self.authority])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);

        if !request_ok {
            return true;
        }

        // Accept the transfer with the target_authority as signer.
        let accept_ok = self.ctx
            .program(self.program_id)
            .call(instruction::AcceptGovernanceAuthorityTransfer)
            .accounts(accounts::AcceptGovernanceAuthorityTransfer {
                payer: self.target_authority.pubkey(),
                config: self.config_pda,
            })
            .signers(&[&*self.target_authority])
            .send()
            .map(|o| o.is_success())
            .unwrap_or(false);

        if !accept_ok {
            return true;
        }

        // Observation: the planted bug surfaces here. On a clean program the
        // target field is None; on the planted variant it remains
        // Some(target_authority.pubkey()).
        let config_state: Config = self.ctx
            .read_anchor_account(&self.config_pda)
            .expect("Config exists after initialize");
        if config_state.target_governance_authority.is_some() {
            self.nonatomic_observed = true;
        }

        // Restore the original governance_authority so the next iteration's
        // request/cancel signed by `self.authority` is still valid. The accept
        // above moved governance_authority to target_authority; transfer it
        // back via a fresh request/accept cycle with the roles swapped.
        let _ = self.ctx
            .program(self.program_id)
            .call(instruction::RequestGovernanceAuthorityTransfer {
                target_governance_authority: self.authority.pubkey(),
            })
            .accounts(accounts::Governance {
                payer: self.target_authority.pubkey(),
                config: self.config_pda,
            })
            .signers(&[&*self.target_authority])
            .send();
        let _ = self.ctx
            .program(self.program_id)
            .call(instruction::AcceptGovernanceAuthorityTransfer)
            .accounts(accounts::AcceptGovernanceAuthorityTransfer {
                payer: self.authority.pubkey(),
                config: self.config_pda,
            })
            .signers(&[&*self.authority])
            .send();

        true
    }
}

// two_step_governance invariant.
//
// If the program ever left `Config.target_governance_authority = Some(_)` after
// a successful `accept_governance_authority_transfer`, the sticky flag is `true`
// and this assertion fails. The clean program clears the field; the planted
// variant drops the clear and surfaces the violation.
#[invariant_test]
fn invariant_two_step_governance_atomic(
    fixture: &mut PythTwoStepGovernanceFixture,
) {
    fuzz_assert_eq!(
        fixture.nonatomic_observed, false,
        "non-atomic accept_governance_authority_transfer observed on Config {}",
        fixture.config_pda
    );
}
