//! Fuzz target: XDR decoder must never panic on untrusted input.
//!
//! Run with:
//!   cargo fuzz run fuzz_xdr_decoder
//!
//! Strategy: split the fuzzer input into three slices and feed them as the
//! transaction envelope, result, and result-meta XDR blobs to
//! `decode_historical_transaction`. Garbage must surface as `Err`, never as a
//! panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroscope_core::xdr_decoder::decode_historical_transaction;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Split input into three roughly equal XDR blobs.
    let third = data.len().div_ceil(3);
    let (envelope_xdr, rest) = data.split_at(third.min(data.len()));
    let (result_xdr, result_meta_xdr) = rest.split_at((third).min(rest.len()));

    let _ = decode_historical_transaction(envelope_xdr, result_xdr, result_meta_xdr);
});
