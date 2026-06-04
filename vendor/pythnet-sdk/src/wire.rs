//! Pyth Wire Format
//!
//! Pyth uses a custom wire format when moving data between programs and chains. This module
//! provides the serialization and deserialization logic for this format as well as definitions of
//! data structures used in the PythNet ecosystem.
//!
//! See the `ser` submodule for a description of the Pyth Wire format.
// CF-PORT: upstream `mod tests` block stripped (golden-buffer + accumulator-fixture tests
//   are not part of the receiver's compile graph); production code 1:1.

pub mod array;
mod de;
mod prefixed_vec;
mod ser;

pub use {
    de::{from_slice, Deserializer, DeserializerError},
    prefixed_vec::PrefixedVec,
    ser::{to_vec, to_writer, Serializer, SerializerError},
};

// Proof Format (V1)
// --------------------------------------------------------------------------------
// The definitions within each module can be updated with append-only data without requiring a new
// module to be defined. So for example, it is possible to add new fields can be added to the end
// of the `AccumulatorAccount` without moving to a `v1`.
pub mod v1 {
    use {
        super::*,
        crate::{
            accumulators::merkle::MerklePath, error::Error, hashers::keccak256_160::Keccak160,
            require,
        },
        borsh::{BorshDeserialize, BorshSerialize},
        serde::{Deserialize, Serialize},
    };
    pub const PYTHNET_ACCUMULATOR_UPDATE_MAGIC: &[u8; 4] = b"PNAU";
    pub const CURRENT_MINOR_VERSION: u8 = 0;

    // Transfer Format.
    // --------------------------------------------------------------------------------
    // This definition is what will be sent over the wire (I.E, pulled from PythNet and submitted
    // to target chains).
    #[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
    pub struct AccumulatorUpdateData {
        magic: [u8; 4],
        major_version: u8,
        minor_version: u8,
        trailing: Vec<u8>,
        pub proof: Proof,
    }

    impl AccumulatorUpdateData {
        pub fn new(proof: Proof) -> Self {
            Self {
                magic: *PYTHNET_ACCUMULATOR_UPDATE_MAGIC,
                major_version: 1,
                minor_version: 0,
                trailing: vec![],
                proof,
            }
        }

        pub fn try_from_slice(bytes: &[u8]) -> Result<Self, Error> {
            let message = from_slice::<byteorder::BE, Self>(bytes)
                .map_err(|_| Error::DeserializationError)?;
            require!(
                &message.magic[..] == PYTHNET_ACCUMULATOR_UPDATE_MAGIC,
                Error::InvalidMagic
            );
            require!(message.major_version == 1, Error::InvalidVersion);
            #[allow(clippy::absurd_extreme_comparisons)]
            {
                require!(
                    message.minor_version >= CURRENT_MINOR_VERSION,
                    Error::InvalidVersion
                );
            }
            Ok(message)
        }
    }

    // A hash of some data.
    pub type Hash = [u8; 20];

    #[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
    pub enum Proof {
        WormholeMerkle {
            vaa: PrefixedVec<u16, u8>,
            updates: Vec<MerklePriceUpdate>,
        },
    }

    #[derive(
        Clone, Debug, Hash, PartialEq, Serialize, Deserialize, BorshDeserialize, BorshSerialize,
    )]
    pub struct MerklePriceUpdate {
        pub message: PrefixedVec<u16, u8>,
        pub proof: MerklePath<Keccak160>,
    }

    #[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
    pub struct WormholeMessage {
        pub magic: [u8; 4],
        pub payload: WormholePayload,
    }

    pub const ACCUMULATOR_UPDATE_WORMHOLE_VERIFICATION_MAGIC: &[u8; 4] = b"AUWV";

    impl WormholeMessage {
        pub fn new(payload: WormholePayload) -> Self {
            Self {
                magic: *ACCUMULATOR_UPDATE_WORMHOLE_VERIFICATION_MAGIC,
                payload,
            }
        }

        pub fn try_from_bytes(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
            let message = from_slice::<byteorder::BE, Self>(bytes.as_ref())
                .map_err(|_| Error::DeserializationError)?;
            require!(
                &message.magic[..] == ACCUMULATOR_UPDATE_WORMHOLE_VERIFICATION_MAGIC,
                Error::InvalidMagic
            );
            Ok(message)
        }
    }

    #[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
    pub enum WormholePayload {
        Merkle(WormholeMerkleRoot),
    }

    #[derive(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
    pub struct WormholeMerkleRoot {
        pub slot: u64,
        pub ring_size: u32,
        pub root: Hash,
    }
}
