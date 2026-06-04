// CF-PORT: CaliperForge-authored stub of wormhole-core-bridge-solana exposing only the
// read-side surface the Pyth receiver imports:
//   - `state::GuardianSet { SEED_PREFIX, index, keys, expiration_time }` + `is_active(&Clock)`
//   - `sdk::legacy::AccountVariant<T>` with `try_deserialize` and `inner()`
//   - `sdk::VaaAccount` with `load`, `try_emitter_address`, `try_emitter_chain`, `try_payload`
//   - `sdk::quorum(usize) -> usize`
//
// The receiver does NOT CPI-call into wormhole — these reads either deserialize from
// `AccountInfo::try_borrow_data()` (GuardianSet) or compute pure on the input (quorum). All
// fixture-side state is injected through Crucible's `write_anchor_account`, so the stub layouts
// must exactly match what the fixtures will pre-write.

#![allow(clippy::result_large_err, unexpected_cfgs)]

use anchor_lang::prelude::*;

// Canonical wormhole core-bridge program ID on Solana mainnet — embedded so the
// `#[account]`-derived owner constraint resolves to a deterministic address (matches
// what the receiver's config.wormhole will be set to in production fixtures).
declare_id!("worm2ZoG2kUd4vFXhvjh93UUH596ayRfgQ2MgjNMTth");

pub mod state {
    use super::*;

    /// Read-side GuardianSet account layout. Fixture seeds this via
    /// `ctx.write_anchor_account(&guardian_set_pubkey, &GuardianSet { ... })`. The
    /// receiver reads `index`, iterates `keys`, and calls `is_active(&clock)`.
    #[account]
    #[derive(Debug, PartialEq)]
    pub struct GuardianSet {
        pub index: u32,
        pub keys: Vec<[u8; 20]>,
        /// Wormhole guardian set "creation time" used by `is_active`. We keep the field
        /// for layout fidelity even though our governance / reclaim fixtures don't
        /// exercise the `is_active` gate.
        pub creation_time: u32,
        /// 0 means "never expires" — matches upstream wormhole's "active forever" sentinel.
        pub expiration_time: u32,
    }

    impl GuardianSet {
        /// PDA seed prefix the receiver checks against in `deserialize_guardian_set_checked`.
        pub const SEED_PREFIX: &'static [u8] = b"GuardianSet";

        /// `true` iff the guardian set has not expired at `timestamp.seconds`. A zero
        /// `expiration_time` is treated as "never expires" (matches upstream's V1
        /// guardian-set semantics — only the latest set has expiration_time = 0).
        pub fn is_active(&self, timestamp: &Timestamp) -> bool {
            self.expiration_time == 0 || timestamp.seconds < i64::from(self.expiration_time)
        }
    }

    /// Minimal stand-in for the upstream `Timestamp` wrapper the receiver passes to
    /// `is_active`. `Clock::get()` is `From`-converted via the impl below.
    #[derive(Copy, Clone, Debug)]
    pub struct Timestamp {
        pub seconds: i64,
    }

    impl From<Clock> for Timestamp {
        fn from(clock: Clock) -> Self {
            Self {
                seconds: clock.unix_timestamp,
            }
        }
    }
}

pub mod sdk {
    use super::*;

    /// Upstream `quorum` is `(2 * total) / 3 + 1`. Receiver uses this only as a
    /// classification threshold to set the `VerificationLevel::{Full, Partial}` tag;
    /// not part of any access-control or conservation gate.
    pub fn quorum(num_guardians: usize) -> usize {
        (num_guardians * 2) / 3 + 1
    }

    pub mod legacy {
        use super::*;

        /// Upstream `AccountVariant` is a thin enum over `V0(T) | V1(T) | ...` used for
        /// migration-compat. The receiver only reads from `inner()` after a successful
        /// `try_deserialize`, so a single-variant newtype is sufficient.
        pub struct AccountVariant<T> {
            inner: T,
        }

        impl<T: AccountDeserialize> AccountVariant<T> {
            pub fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
                let inner = T::try_deserialize(buf)?;
                Ok(Self { inner })
            }

            pub fn inner(&self) -> &T {
                &self.inner
            }
        }
    }

    /// Stub of upstream `VaaAccount`. The receiver loads this in `post_update` and
    /// `post_twap_update`, then reads emitter chain / address / payload. Our two
    /// invariant fixtures (`two_step_governance`, `reclaim_rent_conservation`)
    /// don't drive either of those instructions, so the methods here surface a
    /// well-typed `ReceiverError::DeserializeVaaFailed`-shaped error at runtime.
    /// What matters for the receiver's link is signature + type fidelity. The
    /// `_marker` is `()` rather than a borrow so `load` doesn't impose a lifetime
    /// covariance constraint on the caller's `&AccountInfo<'info>` borrow.
    pub struct VaaAccount {
        _marker: (),
    }

    impl VaaAccount {
        pub fn load(_account_info: &AccountInfo<'_>) -> Result<Self> {
            Err(error!(VaaError::Unsupported))
        }

        pub fn try_emitter_address(&self) -> Result<[u8; 32]> {
            Err(error!(VaaError::Unsupported))
        }

        pub fn try_emitter_chain(&self) -> Result<u16> {
            Err(error!(VaaError::Unsupported))
        }

        pub fn try_payload(&self) -> Result<Vec<u8>> {
            Err(error!(VaaError::Unsupported))
        }
    }

    #[error_code]
    pub enum VaaError {
        /// Surfaces only on the post_update / post_twap_update paths, which the
        /// 2-class harness does not exercise. The error string makes the runtime
        /// failure mode explicit if a future fixture starts driving those ix.
        #[msg("VaaAccount::load is stubbed in cf-invariants-pyth (post_update path unused by 2-class harness)")]
        Unsupported,
    }
}
