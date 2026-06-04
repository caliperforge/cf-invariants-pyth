//! A MerkleTree based Accumulator.
//!
// CF-PORT: upstream `mod test` block stripped (depended on proptest); production code is 1:1.

use {
    crate::{
        accumulators::Accumulator,
        hashers::{keccak256::Keccak256, Hasher},
    },
    borsh::{BorshDeserialize, BorshSerialize},
    serde::{Deserialize, Serialize},
};

// We need to discern between leaf and intermediate nodes to prevent trivial second pre-image
// attacks. If we did not do this it would be possible for an attacker to intentionally create
// non-leaf nodes that have the same hash as a leaf node, and then use that to prove the existence
// of a leaf node that does not exist.
//
// See:
//
// - https://flawed.net.nz/2018/02/21/attacking-merkle-trees-with-a-second-preimage-attack
// - https://en.wikipedia.org/wiki/Merkle_tree#Second_preimage_attack
//
// NOTE: We use a NULL prefix for leaf nodes to distinguish them from the empty message (""), while
// there is no path that allows empty messages this is a safety measure to prevent future
// vulnerabilities being introduced.
const LEAF_PREFIX: &[u8] = &[0];
const NODE_PREFIX: &[u8] = &[1];
const NULL_PREFIX: &[u8] = &[2];

/// A MerklePath contains a list of hashes that form a proof for membership in a tree.
#[derive(
    Clone,
    Default,
    Debug,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct MerklePath<H: Hasher>(Vec<H::Hash>);

/// A MerkleRoot contains the root hash of a MerkleTree.
#[derive(
    Clone,
    Default,
    Debug,
    Hash,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct MerkleRoot<H: Hasher>(H::Hash);

/// A MerkleTree is a binary tree where each node is the hash of its children.
#[derive(
    Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize, Serialize, Deserialize, Default,
)]
pub struct MerkleTree<H: Hasher = Keccak256> {
    pub root: MerkleRoot<H>,

    // CF-PORT: upstream uses borsh-0.10's bare `#[borsh_skip]`; borsh-1.6 spells this
    // `#[borsh(skip)]` (the helper-attr was reshaped to a structured form).
    #[serde(skip)]
    #[borsh(skip)]
    pub nodes: Vec<H::Hash>,
}

/// Implements functionality for using standalone MerkleRoots.
impl<H: Hasher> MerkleRoot<H> {
    /// Construct a MerkleRoot from an existing Hash.
    pub fn new(root: H::Hash) -> Self {
        Self(root)
    }

    /// Given a item and corresponding MerklePath, check that it is a valid membership proof.
    pub fn check(&self, proof: MerklePath<H>, item: &[u8]) -> bool {
        let mut current: <H as Hasher>::Hash = MerkleTree::<H>::hash_leaf(item);
        for hash in proof.0 {
            current = MerkleTree::<H>::hash_node(&current, &hash);
        }
        current == self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// Implements functionality for working with MerklePath (proofs).
impl<H: Hasher> MerklePath<H> {
    /// Given a Vector of hashes representing a merkle proof, construct a MerklePath.
    pub fn new(path: Vec<H::Hash>) -> Self {
        Self(path)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.0
            .iter()
            .flat_map(|hash| hash.as_ref().to_vec())
            .collect()
    }

    pub fn to_vec(&self) -> Vec<H::Hash> {
        self.0.clone()
    }
}

/// Presents an Accumulator friendly interface for MerkleTree.
impl<'a, H: Hasher + 'a> Accumulator<'a> for MerkleTree<H> {
    type Proof = MerklePath<H>;

    /// Construct a MerkleTree from an iterator of items.
    fn from_set(items: impl Iterator<Item = &'a [u8]>) -> Option<Self> {
        let items: Vec<&[u8]> = items.collect();
        Self::new(&items)
    }

    /// Prove an item is in the tree by returning a MerklePath.
    fn prove(&'a self, item: &[u8]) -> Option<Self::Proof> {
        let item = MerkleTree::<H>::hash_leaf(item);
        let index = self.nodes.iter().position(|i| i == &item)?;
        Some(self.find_path(index))
    }

    // NOTE: This `check` call is intended to fit the generic accumulator implementation, but for a
    // merkle tree the proof does not usually need the `self` parameter as the proof is standalone
    // and doesn't need the original nodes.
    fn check(&'a self, proof: Self::Proof, item: &[u8]) -> bool {
        self.verify_path(proof, item)
    }
}

/// Implement a MerkleTree-specific interface for interacting with trees.
impl<H: Hasher> MerkleTree<H> {
    /// Construct a new MerkleTree from a list of byte slices.
    ///
    /// This list does not have to be a set which means the tree may contain duplicate items. It is
    /// up to the caller to enforce a strict set-like object if that is desired.
    pub fn new(items: &[&[u8]]) -> Option<Self> {
        if items.is_empty() {
            return None;
        }

        let depth = items.len().next_power_of_two().trailing_zeros();
        let mut tree: Vec<H::Hash> = vec![Default::default(); 1 << (depth + 1)];

        // Filling the leaf hashes
        for i in 0..(1 << depth) {
            if i < items.len() {
                tree[(1 << depth) + i] = MerkleTree::<H>::hash_leaf(items[i]);
            } else {
                tree[(1 << depth) + i] = MerkleTree::<H>::hash_null();
            }
        }

        // Filling the node hashes from bottom to top
        for k in (1..=depth).rev() {
            let level = k - 1;
            let level_num_nodes = 1 << level;
            for i in 0..level_num_nodes {
                let id = (1 << level) + i;
                tree[id] = MerkleTree::<H>::hash_node(&tree[id * 2], &tree[id * 2 + 1]);
            }
        }

        Some(Self {
            root: MerkleRoot::new(tree[1]),
            nodes: tree,
        })
    }

    /// Produces a Proof of membership for an index in the tree.
    pub fn find_path(&self, mut index: usize) -> MerklePath<H> {
        let mut path = Vec::new();
        while index > 1 {
            path.push(self.nodes[index ^ 1]);
            index /= 2;
        }
        MerklePath::new(path)
    }

    /// Check if a given MerklePath is a valid proof for a corresponding item.
    pub fn verify_path(&self, proof: MerklePath<H>, item: &[u8]) -> bool {
        self.root.check(proof, item)
    }

    #[inline]
    pub fn hash_leaf(leaf: &[u8]) -> H::Hash {
        H::hashv(&[LEAF_PREFIX, leaf])
    }

    #[inline]
    pub fn hash_node(l: &H::Hash, r: &H::Hash) -> H::Hash {
        H::hashv(&[
            NODE_PREFIX,
            (if l <= r { l } else { r }).as_ref(),
            (if l <= r { r } else { l }).as_ref(),
        ])
    }

    #[inline]
    pub fn hash_null() -> H::Hash {
        H::hashv(&[NULL_PREFIX])
    }

    /// Serialize a MerkleTree into a Vec<u8>.
    ///
    ///Layout:
    ///
    /// ```rust,ignore
    /// 4 bytes:  magic number
    /// 1 byte:   update type
    /// 4 byte:   storage id
    /// 32 bytes: root hash
    /// ```
    ///
    /// TODO: This code does not belong to MerkleTree, we should be using the wire data types in
    /// calling code to wrap this value.
    pub fn serialize(&self, slot: u64, ring_size: u32) -> Vec<u8> {
        let mut serialized = vec![];
        serialized.extend_from_slice(0x41555756u32.to_be_bytes().as_ref());
        serialized.extend_from_slice(0u8.to_be_bytes().as_ref());
        serialized.extend_from_slice(slot.to_be_bytes().as_ref());
        serialized.extend_from_slice(ring_size.to_be_bytes().as_ref());
        serialized.extend_from_slice(self.root.0.as_ref());
        serialized
    }
}
