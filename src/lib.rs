//! `jblock` — a Java thread dump parser/analyzer exposed to JavaScript via WebAssembly.
//!
//! The heavy lifting lives in [`parser`], which is a plain-Rust module so that
//! it can be unit-tested with `cargo test`. This file only provides the wasm bindings.
//!
//! Java reproducer generation (feat-007) lives in [`codegen`] on the host target
//! for tests/`examples/gen_java`, and in `web/src/codegen.ts` for the page
//! (feat-011) so it is not shipped inside the WASM binary.
//!
//! The optional host CLI (`cargo run --features cli --bin jblock`, feat-056)
//! wraps the same parser for file / stdin / clipboard input.

#[cfg(not(target_arch = "wasm32"))]
pub mod capture;
#[cfg(all(not(target_arch = "wasm32"), feature = "cli"))]
pub mod cli;
#[cfg(not(target_arch = "wasm32"))]
pub mod codegen;
pub mod gc;
pub mod input_kind;
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

/// Which analyzers an input text should feed: `{ thread_dump, gc_log }` (feat-074).
#[wasm_bindgen(js_name = detectInputKinds)]
pub fn detect_input_kinds(text: &str) -> Result<JsValue, JsValue> {
    to_js(&input_kind::detect_input_kinds(text))
}

/// A GC analysis session kept inside the analyze worker (feat-074).
///
/// Holds the imported texts and their stitched parse so range statistics,
/// chart columns and event pages are computed on demand without shipping
/// every event to the UI thread.
#[wasm_bindgen]
pub struct GcSession {
    names: Vec<String>,
    texts: Vec<String>,
    stitched: gc::stitch::Stitched,
}

#[derive(serde::Deserialize)]
struct NamedText {
    name: String,
    text: String,
}

fn to_js<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true).serialize_missing_as_null(true))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

impl GcSession {
    fn restitch(&mut self, mode: gc::stitch::GroupingMode) {
        let inputs: Vec<gc::stitch::GcInput> = self
            .names
            .iter()
            .zip(&self.texts)
            .map(|(name, text)| gc::stitch::GcInput { name, text })
            .collect();
        self.stitched = gc::stitch::stitch(&inputs, mode);
    }

    fn group(&self, group: usize) -> Result<&gc::stitch::GcGroup, JsValue> {
        self.stitched.groups.get(group).ok_or_else(|| JsValue::from_str("no such GC group"))
    }
}

#[wasm_bindgen]
impl GcSession {
    /// `files`: JS array of `{ name, text }`; `mode`: `auto` | `merge` | `split`.
    #[wasm_bindgen(constructor)]
    pub fn new(files: JsValue, mode: &str) -> Result<GcSession, JsValue> {
        let files: Vec<NamedText> = serde_wasm_bindgen::from_value(files)
            .map_err(|e| JsValue::from_str(&format!("GcSession expects {{name,text}}[]: {e}")))?;
        let mode = gc::stitch::GroupingMode::parse(mode).ok_or_else(|| JsValue::from_str("mode must be auto|merge|split"))?;
        let (names, texts) = files.into_iter().map(|f| (f.name, f.text)).unzip();
        let mut session = GcSession { names, texts, stitched: gc::stitch::stitch(&[], mode) };
        session.restitch(mode);
        Ok(session)
    }

    /// Regroup the same files with another mode.
    pub fn regroup(&mut self, mode: &str) -> Result<(), JsValue> {
        let mode = gc::stitch::GroupingMode::parse(mode).ok_or_else(|| JsValue::from_str("mode must be auto|merge|split"))?;
        self.restitch(mode);
        Ok(())
    }

    /// Groups, JVM info, whole-log KPIs, findings and diagnostics (no per-event data).
    pub fn report(&self) -> Result<JsValue, JsValue> {
        to_js(&gc::report::build_report(&self.stitched))
    }

    /// Names of the chart columns available through [`GcSession::column`].
    #[wasm_bindgen(js_name = columnNames)]
    pub fn column_names() -> Vec<String> {
        gc::report::SERIES_COLUMNS.iter().chain(gc::report::DERIVED_COLUMNS.iter()).map(|s| s.to_string()).collect()
    }

    /// One chart column as a `Float64Array` (NaN = missing).
    pub fn column(&self, group: usize, name: &str) -> Result<Vec<f64>, JsValue> {
        gc::report::series_column(&self.group(group)?.log.events, name)
            .ok_or_else(|| JsValue::from_str(&format!("unknown GC column {name}")))
    }

    /// KPIs, histogram and causes for `[t0, t1]` on the group's time axis.
    pub fn stats(&self, group: usize, t0: f64, t1: f64) -> Result<JsValue, JsValue> {
        let log = &self.group(group)?.log;
        to_js(&gc::stats::compute(&log.events, Some((t0, t1)), log.jvm.heap_max_kb))
    }

    /// A page of events matching `query` (see `EventQuery`).
    pub fn events(&self, group: usize, query: JsValue) -> Result<JsValue, JsValue> {
        let q: gc::report::EventQuery = if query.is_undefined() || query.is_null() {
            Default::default()
        } else {
            serde_wasm_bindgen::from_value(query).map_err(|e| JsValue::from_str(&format!("bad event query: {e}")))?
        };
        to_js(&gc::report::query_events(&self.group(group)?.log.events, &q))
    }
}
