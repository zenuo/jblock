//! `jblock` — a Java thread dump parser/analyzer exposed to JavaScript via WebAssembly.
//!
//! The heavy lifting lives in [`parser`], which is a plain-Rust module so it can
//! be unit-tested with `cargo test`. This file only provides the wasm bindings.

mod parser;

pub use parser::{analyze, Analysis, BlockedEdge, DumpFormat, StateCount, ThreadInfo};

use wasm_bindgen::prelude::*;

/// Initialise wasm-side hooks. Safe to call multiple times.
#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Parse and analyze a Java thread dump.
///
/// Accepts both `jstack` output and `ThreadMXBean#dumpAllThreads` output.
/// Returns a plain JS object matching the `Analysis` shape (see `web/src/types.ts`).
#[wasm_bindgen(js_name = analyzeDump)]
pub fn analyze_dump(input: &str) -> Result<JsValue, JsValue> {
    let analysis = parser::analyze(input);
    serde_wasm_bindgen::to_value(&analysis).map_err(|e| JsValue::from_str(&e.to_string()))
}
