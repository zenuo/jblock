//! `jblock` — a Java thread dump parser/analyzer exposed to JavaScript via WebAssembly.
//!
//! The heavy lifting lives in [`parser`], which is a plain-Rust module so it can
//! be unit-tested with `cargo test`. This file only provides the wasm bindings.

pub mod codegen;
mod parser;

pub use codegen::{generate as generate_java_source, parse_scenario, Scenario};
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

/// Generate a runnable Java reproducer for a thread problem scenario.
///
/// `scenario` is one of `"lock-contention"` or `"deadlock"` (aliases accepted);
/// `count` is the number of threads (clamped to 2..=64). Returns Java source.
#[wasm_bindgen(js_name = generateJava)]
pub fn generate_java(scenario: &str, count: usize) -> Result<String, JsValue> {
    let scenario = codegen::parse_scenario(scenario)
        .ok_or_else(|| JsValue::from_str(&format!("unknown scenario: {scenario}")))?;
    Ok(codegen::generate(scenario, count))
}
