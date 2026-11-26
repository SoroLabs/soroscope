//! Fuzz target: WASM parser must never panic on untrusted input.
//!
//! Run with:
//!   cargo fuzz run fuzz_wasm_parser
//!
//! Strategy: feed the raw fuzzer bytes straight into `parse_module` (and the
//! security `Scanner` on successfully parsed modules). Malformed modules must
//! surface as `Err`/empty findings, never as a panic.
#![no_main]

use libfuzzer_sys::fuzz_target;
use soroscope_core::parsed_module::parse_module;
use soroscope_core::parser::Scanner;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let Ok(_module) = parse_module(data) else {
        return;
    };

    // Scan the successfully parsed module — finding collection must not panic.
    let scanner = Scanner::new();
    let _ = scanner.scan(data);
});
