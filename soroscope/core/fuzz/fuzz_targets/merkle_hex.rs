//! Fuzz target: feed arbitrary strings to MerkleTree::from_hex_strings.
//!
//! Run with:
//!   cargo fuzz run merkle_hex
//!
//! Exercises the hex-decode path with random byte content including
//! invalid UTF-8 representations and odd-length hex strings.
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroscope_core::merkle_tree::MerkleTree;

fuzz_target!(|data: &[u8]| {
    // Interpret the input as a sequence of hex strings separated by 0x00.
    let hex_strings: Vec<String> = data
        .split(|&b| b == 0x00)
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();

    if hex_strings.is_empty() {
        return;
    }

    // from_hex_strings must never panic regardless of input validity.
    let _ = MerkleTree::from_hex_strings(hex_strings);
});
