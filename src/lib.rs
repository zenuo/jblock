//! `jblock` — a Java thread dump parser/analyzer exposed to JavaScript via WebAssembly.
//!
//! The heavy lifting lives in [`parser`], which is a plain-Rust module so it can
//! be unit-tested with `cargo test`. This file only provides the wasm bindings.
//!
//! Java reproducer generation (feat-007) lives in [`codegen`] on the host target
//! for tests/`examples/gen_java`, and in `web/src/codegen.ts` for the page
//! (feat-011) so it is not shipped inside the WASM binary.

#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
#[cfg(not(target_arch = "wasm32"))]
pub mod codegen;
mod parser;

#[cfg(not(target_arch = "wasm32"))]
pub use codegen::{generate as generate_java_source, parse_scenario, Scenario};
pub use parser::{
    analyze, analyze_series, Analysis, BlockedEdge, Deadlock, DumpFormat, MultiDumpAnalysis,
    PatternHit, PatternKind, StateCount, ThreadInfo,
};

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

/// Analyze an ordered series of dumps for cross-dump patterns (feat-041).
///
/// `inputs` must be a JS array of strings. Returns `{ dumps, cross_patterns }`.
#[wasm_bindgen(js_name = analyzeDumps)]
pub fn analyze_dumps(inputs: JsValue) -> Result<JsValue, JsValue> {
    let texts: Vec<String> = serde_wasm_bindgen::from_value(inputs)
        .map_err(|e| JsValue::from_str(&format!("analyzeDumps expects string[]: {e}")))?;
    if texts.is_empty() {
        return Err(JsValue::from_str("analyzeDumps requires at least one dump"));
    }
    let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let result = parser::analyze_series(&refs);
    serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}
