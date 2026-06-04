# cf-invariants-pyth

[![ci](https://github.com/caliperforge/cf-invariants-pyth/actions/workflows/ci.yml/badge.svg)](https://github.com/caliperforge/cf-invariants-pyth/actions/workflows/ci.yml)

**An invariant-fuzzing harness for the [Pyth Solana Receiver](https://github.com/pyth-network/pyth-crosschain/tree/main/target_chains/solana/programs/pyth-solana-receiver), run on [Crucible](https://github.com/asymmetric-research/crucible).**

cf-invariants-pyth is a focused harness, not a new fuzzer. It ports the
upstream Pyth Solana Receiver program from `anchor-lang` 0.32.1 to
`anchor-lang` 1.0.1 so it can be driven by Crucible v0.2.0 (LibAFL +
LiteSVM), then runs three invariant classes against a clean reference
and planted-bug twins. Every push, CI rebuilds all variants and asserts
`clean = 0` violations and `planted >= 1` violation per class.

This is a sibling artifact to
[cf-invariants-jito](https://github.com/caliperforge/cf-invariants-jito),
[cf-invariants-jito-tippayment](https://github.com/caliperforge/cf-invariants-jito-tippayment),
[cf-invariants-jito-priorityfee](https://github.com/caliperforge/cf-invariants-jito-priorityfee),
and [cf-invariants-anchor](https://github.com/caliperforge/cf-invariants-anchor)
(the generic Anchor / Crucible invariant-author scaffold), shipped by the same
operator.

> **Status:** scaffold (LICENSE / NOTICE / README / CI composite action /
> workspace shell). The receiver port from anchor 0.32.1 to 1.0.1 + vendored
> SDK crates + invariant fuzz fixtures land in subsequent commits. CI is
> not green yet — the workspace is empty of program code. See the
> `Project status` section below.

## Scope — what Pyth Solana Receiver is, what this harness covers

The Pyth Solana Receiver is the on-chain piece of the
[Pyth Network](https://www.pyth.network/) price-feed delivery stack on
Solana. It accepts attested price updates (delivered as Wormhole VAAs +
Merkle proofs) and writes them into `PriceUpdateV2` / `TwapUpdate`
accounts that downstream Solana DeFi protocols consume. Pyth is the de
facto Solana oracle, used by Drift, marginfi, Kamino, and most other
major Solana DeFi protocols. The upstream code lives at
`pyth-network/pyth-crosschain/target_chains/solana/programs/pyth-solana-receiver`
and is licensed Apache-2.0.

This harness does not modify the production program. It targets the
**invariant surface** of that program — the structural properties that
must hold no matter what call sequence is fuzzed — and proves the
harness can both confirm those properties on the clean reference and
catch a deliberately planted regression in each class.

## What it tests — three invariant classes (planned)

Each invariant runs as a Crucible fuzz fixture against (a) the clean
reference and (b) a single-site planted-bug twin. CI asserts
`clean = 0` violations and `planted >= 1` violation per class.

| # | Class | Invariant under test | Planted-bug site |
|---|---|---|---|
| 1 | `two_step_governance` | `invariant_governance_two_step_atomic` — only the current `governance_authority` can mutate config, AND `accept_governance_authority_transfer` atomically clears `target_governance_authority` (no replay of an accepted transfer). | `accept_governance_authority_transfer` — drop the `target_governance_authority = None` reset. |
| 2 | `vaa_quorum_gating` | `invariant_post_update_requires_quorum` — `post_update_atomic` rejects any VAA whose signature count is below `config.minimum_signatures`. | `post_update_atomic` — drop the `require_gte!(vaa.signature_count(), config.minimum_signatures, ...)` gate. |
| 3 | `reclaim_rent_conservation` | `invariant_reclaim_rent_no_double_close` — a `reclaim_rent` / `reclaim_twap_rent` call closes the update account to the rightful payer and cannot be replayed against the same account. | `ReclaimRent` — drop the `constraint = price_update_account.write_authority == payer.key()` check. |

These three classes were selected because all three hit the receiver's
clean fits per the feasibility recon:
`agents/research_lead/outbox/solana_4th_protocol_feasibility_2026-06-04.md`
§11.

Once the port lands, the AI suggester (`agents/build_squad_lead`-owned,
shared with cf-invariants-anchor / cf-invariants-jito) runs over the
ported source and tags any additional candidates as
`InvariantSource::AiSuggested`.

## Repository layout (planned)

```
.
├── programs/
│   └── pyth-solana-receiver/         # ported from pyth-network/pyth-crosschain (anchor-lang 1.0.1)
├── vendor/
│   ├── pyth-solana-receiver-sdk/     # minimal vendored copy of upstream SDK (Config / state types / IDs)
│   ├── pythnet-sdk/                  # minimal vendored copy of pythnet/pythnet_sdk (merkle + messages + wire)
│   └── wormhole-core-bridge-stub/    # CaliperForge-authored stub for GuardianSet / VaaAccount / quorum
├── references/
│   ├── pyth_receiver_ref/                            # clean baseline + 3 Crucible fuzz fixtures
│   ├── pyth_receiver_ref_planted_two_step_governance/        # planted #1
│   ├── pyth_receiver_ref_planted_vaa_quorum_gating/          # planted #2
│   └── pyth_receiver_ref_planted_reclaim_rent_conservation/  # planted #3
├── .github/workflows/ci.yml          # CI: workspace check + build-sbf + harness matrix
├── .github/actions/setup-rust/       # composite action (no per-job prelude copy-paste)
├── Cargo.toml                        # workspace
├── LICENSE                           # Apache-2.0 (CaliperForge)
├── NOTICE                            # Pyth attribution + modification log
└── README.md
```

## Project status

Phase 1, scaffold (Day 4 of the route, 2026-06-04):

- [x] LICENSE / NOTICE / README born-with the repo (§0 publish_cascade).
- [x] Composite `setup-rust` action committed (no per-job prelude copy-paste).
- [ ] Receiver port from anchor 0.32.1 → 1.0.1.
- [ ] Vendored `pyth_solana_receiver_sdk` (Config + state types + IDs).
- [ ] Vendored `pythnet_sdk` (merkle + message + wire types).
- [ ] CaliperForge-authored `wormhole-core-bridge-stub` (GuardianSet read surface).
- [ ] Three invariant fuzz fixtures (clean + planted variants for each).
- [ ] CI green (build-sbf + clean+planted matrix).
- [ ] AI-suggester live run + `InvariantSource::AiSuggested` tags.

See `agents/rust_anchor_specialist/outbox/cf_invariants_pyth_phase1_2026-06-04.md`
for the operator's honest-bound report on remaining work.

## Pinned toolchain

These are the versions CI builds against on every push (see
[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)). Pins were
empirically verified against each upstream's `Cargo.toml`, not eyeballed:

- Rust **stable**.
- `anchor-lang` **1.0.1** — matches Crucible v0.2.0's workspace.
- Upstream [Crucible](https://github.com/asymmetric-research/crucible) **v0.2.0** — built from source in CI.
- Anza / Solana CLI **v2.1.21** for `cargo-build-sbf`.
- Solana platform-tools **v1.52**.

## What this is not

- **Not a fork of Crucible.** Crucible is the harness; cf-invariants-pyth
  is a target + fuzz fixtures that run on top of it. Credit for the
  LiteSVM execution rails and the IDL-driven fuzzing plumbing belongs to
  Asymmetric Research.
- **Not a Pyth security audit.** Each planted twin is a synthetic
  single-site regression authored to prove the corresponding invariant
  class fires. No claim is made about the production Pyth Solana
  Receiver's security from this harness alone.
- **Not a formal-verification tool.** Randomized invariant fuzzing,
  not proofs.

## Credits

- Upstream Pyth Solana Receiver: [Pyth Data Association](https://www.pyth.network/) — `pyth-network/pyth-crosschain` (Apache-2.0).
- Fuzz harness: [Crucible](https://github.com/asymmetric-research/crucible) by [Asymmetric Research](https://www.asymmetric.re/) (MIT, v0.2.0).
- Anchor framework: [coral-xyz/anchor](https://github.com/coral-xyz/anchor) (Apache-2.0).

## Reporting issues, security contact

Open an issue on this GitHub repository, or contact
[michael@caliperforge.com](mailto:michael@caliperforge.com).

## License

Apache-2.0. See [`LICENSE`](./LICENSE) and [`NOTICE`](./NOTICE). The
`NOTICE` file preserves Pyth's upstream Apache-2.0 attribution and
describes the modifications relative to upstream.

---

cf-invariants-pyth is operated by Michael Moffett under the CaliperForge banner. CaliperForge is a sole-operator engineering studio.

This scaffold was built with AI assistance. Authored and reviewed by Michael Moffett, operator at CaliperForge. Full policy at [caliperforge.com/ai-disclosure](https://caliperforge.com/ai-disclosure).
