//! GC parser tests against real JVM logs captured by `scripts/capture-gc-logs.sh`.
//!
//! Expected counts were taken independently with `grep` over the fixtures
//! (summary lines on the bare `gc` tag ending in a duration).

use super::*;

const G1_DEFAULT: &str = include_str!("../../tests/fixtures/gc/g1-default-jdk21.log");
const G1_STRESS: &str = include_str!("../../tests/fixtures/gc/g1-stress-jdk21.log");
const G1_UTC_PID: &str = include_str!("../../tests/fixtures/gc/g1-utctime-pid-jdk21.log");
const G1_GC_ONLY: &str = include_str!("../../tests/fixtures/gc/g1-gc-only-jdk21.log");
const PARALLEL: &str = include_str!("../../tests/fixtures/gc/parallel-jdk21.log");
const SERIAL: &str = include_str!("../../tests/fixtures/gc/serial-jdk21.log");

fn count(log: &ParsedGcLog, pred: impl Fn(&GcEvent) -> bool) -> usize {
    log.events.iter().filter(|e| pred(e)).count()
}

fn assert_sorted(log: &ParsedGcLog) {
    for w in log.events.windows(2) {
        assert!(w[0].time_ms <= w[1].time_ms, "events must be time-ordered");
    }
}

#[test]
fn gc_unified_detects_format_on_all_fixtures() {
    for text in [G1_DEFAULT, G1_STRESS, G1_UTC_PID, G1_GC_ONLY, PARALLEL, SERIAL] {
        assert_eq!(detect_format(text), GcLogFormat::Unified);
    }
    assert_eq!(detect_format(include_str!("../../tests/fixtures/deadlock_real_jstack.txt")), GcLogFormat::Unknown);
    assert_eq!(detect_format("hello\nworld\n"), GcLogFormat::Unknown);
}

#[test]
fn gc_unified_g1_stress_events_and_flags() {
    let log = parse_log(G1_STRESS);
    assert_eq!(log.format, GcLogFormat::Unified);
    assert_eq!(log.collector, Collector::G1);
    assert_eq!(log.events.len(), 147);
    assert_eq!(count(&log, |e| e.is_pause), 107);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 10);
    assert_eq!(count(&log, |e| e.flags.evacuation_failure), 9);
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 40);
    assert!(count(&log, |e| e.kind == GcKind::Mixed) > 0);
    assert!(count(&log, |e| e.kind == GcKind::Remark) > 0);
    assert!(count(&log, |e| e.kind == GcKind::Cleanup) > 0);
    assert!(count(&log, |e| e.cause.as_deref() == Some("System.gc()")) > 0);
    assert!(count(&log, |e| e.cause.as_deref() == Some("G1 Humongous Allocation")) > 0);
    assert_eq!(log.unparsed_count, 0, "samples: {:?}", log.unparsed_samples);
    assert_sorted(&log);

    // Every pause carries heap, CPU and both clocks; young pauses carry region-derived generations.
    for e in log.events.iter().filter(|e| e.is_pause) {
        assert!(e.heap.is_some(), "pause without heap at line {}", e.line);
        assert!(e.cpu.is_some(), "pause without cpu at line {}", e.line);
        assert!(e.uptime_ms.is_some() && e.wall_ms.is_some());
    }
    let young = log.events.iter().find(|e| e.kind == GcKind::Young).unwrap();
    assert!(young.young.is_some() && young.old.is_some() && young.humongous.is_some() && young.metaspace.is_some());

    // GC(57): Prepare Mixed with evacuation failure, 60M->35M(64M) 0.625ms, regions with 1M region size.
    let e = log.events.iter().find(|e| e.gc_id == Some(57) && e.is_pause).unwrap();
    assert_eq!(e.name, "Pause Young (Prepare Mixed)");
    assert_eq!(e.kind, GcKind::Young);
    assert_eq!(e.cause.as_deref(), Some("G1 Evacuation Pause"));
    assert!(e.flags.evacuation_failure);
    assert_eq!(e.duration_ms, 0.625);
    let heap = e.heap.unwrap();
    assert_eq!((heap.before_kb, heap.after_kb, heap.capacity_after_kb), (60.0 * 1024.0, 35.0 * 1024.0, Some(64.0 * 1024.0)));
    let old = e.old.unwrap();
    assert_eq!((old.before_kb, old.after_kb), (27.0 * 1024.0, 36.0 * 1024.0));
    assert_eq!(e.young.unwrap().before_kb, 21.0 * 1024.0); // Eden 20 + Survivor 1
    assert_eq!(e.humongous.unwrap().before_kb, 15.0 * 1024.0);
    // Starts at the gc,start line (8.532s), not at the summary line (8.533s).
    assert_eq!(e.uptime_ms, Some(8532.0));

    // GC(29): concurrent cycle spans its own Remark and Cleanup pauses.
    let ids29: Vec<_> = log.events.iter().filter(|e| e.gc_id == Some(29)).collect();
    let names: Vec<_> = ids29.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["Concurrent Mark Cycle", "Pause Remark", "Pause Cleanup"]);
    assert_eq!(ids29[0].uptime_ms, Some(4642.0));
    assert_eq!(ids29[0].duration_ms, 1.587);
    assert!(!ids29[0].is_pause);
}

#[test]
fn gc_unified_g1_init_facts() {
    let log = parse_log(G1_STRESS);
    assert_eq!(log.jvm.version.as_deref(), Some("21.0.12.1+1-LTS (release)"));
    assert_eq!(log.jvm.heap_max_kb, Some(64.0 * 1024.0));
    assert_eq!(log.jvm.region_size_kb, Some(1024.0));
    assert!(log.jvm.facts.iter().any(|f| f.key == "CPUs"));
    assert!(log.jvm.facts.iter().any(|f| f.key == "Collector" && f.value == "G1"));
}

#[test]
fn gc_unified_default_decorators() {
    let log = parse_log(G1_DEFAULT);
    assert_eq!(log.collector, Collector::G1);
    assert_eq!(log.events.len(), 7);
    assert!(log.events.iter().all(|e| e.uptime_ms.is_some() && e.wall_ms.is_none()));
    assert!(!log.missing_timestamps);
    assert_eq!(log.unparsed_count, 0, "samples: {:?}", log.unparsed_samples);
}

#[test]
fn gc_unified_utctime_pid_tid_decorators() {
    let log = parse_log(G1_UTC_PID);
    assert_eq!(log.events.len(), 5);
    assert!(log.pid.is_some());
    // No uptime decorator: the time axis is wall clock relative to the first event.
    assert!(log.events.iter().all(|e| e.uptime_ms.is_none() && e.wall_ms.is_some()));
    assert_eq!(log.events[0].time_ms, 0.0);
    assert!(log.events.last().unwrap().time_ms > 0.0);
    assert_eq!(log.unparsed_count, 0, "samples: {:?}", log.unparsed_samples);
}

#[test]
fn gc_unified_gc_tag_only_uptimemillis() {
    let log = parse_log(G1_GC_ONLY);
    assert_eq!(log.collector, Collector::G1);
    assert_eq!(log.events.len(), 59);
    assert_eq!(count(&log, |e| e.is_pause), 46);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 3);
    assert_eq!(count(&log, |e| e.flags.evacuation_failure), 1);
    // Without gc,heap lines only the whole-heap change is known.
    assert!(log.events.iter().filter(|e| e.is_pause).all(|e| e.heap.is_some() && e.young.is_none()));
    let first = &log.events[0];
    assert_eq!(first.name, "Pause Young (Normal)");
    // [559ms] summary, 0.384ms pause, no gc,start line → start = end - duration.
    assert!((first.uptime_ms.unwrap() - (559.0 - 0.384)).abs() < 1e-9);
}

#[test]
fn gc_unified_parallel_generations() {
    let log = parse_log(PARALLEL);
    assert_eq!(log.collector, Collector::Parallel);
    assert_eq!(log.events.len(), 26);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 4);
    assert_eq!(count(&log, |e| e.cause.as_deref() == Some("Allocation Failure")), 18);
    assert_eq!(count(&log, |e| e.cause.as_deref() == Some("System.gc()")), 8);
    // GC(3): PSYoungGen: 20995K(28672K)->4083K(28672K), ParOldGen: 2098K(65536K)->3275K(65536K).
    let e = log.events.iter().find(|e| e.gc_id == Some(3)).unwrap();
    assert_eq!(e.name, "Pause Young");
    let young = e.young.unwrap();
    assert_eq!((young.before_kb, young.after_kb, young.capacity_before_kb), (20995.0, 4083.0, Some(28672.0)));
    let old = e.old.unwrap();
    assert_eq!((old.before_kb, old.after_kb, old.capacity_after_kb), (2098.0, 3275.0, Some(65536.0)));
    assert!(e.metaspace.is_some() && e.cpu.is_some());
    assert_eq!(log.unparsed_count, 0, "samples: {:?}", log.unparsed_samples);
}

#[test]
fn gc_unified_serial_generations() {
    let log = parse_log(SERIAL);
    assert_eq!(log.collector, Collector::Serial);
    assert_eq!(log.events.len(), 16);
    assert!(log.events.iter().all(|e| e.young.is_some() && e.old.is_some()));
    // uptimemillis decorator.
    assert!(log.events.iter().all(|e| e.uptime_ms.is_some()));
    assert_eq!(log.unparsed_count, 0, "samples: {:?}", log.unparsed_samples);
}

#[test]
fn gc_unified_noise_lines_are_counted_not_fatal() {
    let text = format!("app started\n{}\nException in thread main\n", G1_DEFAULT);
    let log = parse_log(&text);
    assert_eq!(log.events.len(), 7);
    assert_eq!(log.unparsed_count, 2);
    assert_eq!(log.unparsed_samples[0], (1, "app started".to_string()));
}

#[test]
fn gc_unified_collector_inferred_without_using_line() {
    // A rotated segment has no `Using …` header: infer from Parallel generation lines.
    let body: String = PARALLEL.lines().filter(|l| !l.contains("Using Parallel")).collect::<Vec<_>>().join("\n");
    assert_eq!(parse_log(&body).collector, Collector::Parallel);
    let body: String = G1_GC_ONLY.lines().filter(|l| !l.contains("Using G1")).collect::<Vec<_>>().join("\n");
    assert_eq!(parse_log(&body).collector, Collector::G1);
}

#[test]
fn gc_unified_truncated_and_garbage_input_does_not_panic() {
    for cut in [1, 100, 1000, 5000, G1_STRESS.len() / 2] {
        let mut end = cut.min(G1_STRESS.len());
        while !G1_STRESS.is_char_boundary(end) {
            end -= 1;
        }
        let _ = parse_log(&G1_STRESS[..end]);
    }
    for junk in ["[", "[]", "[[[", "[0s] GC(", "[0s][gc] GC(1) Pause", "[0s][gc] GC(x) Pause Young 1M->", "[1ms] GC(1) Pause Full (System.gc( 3ms"] {
        let _ = parse_log(junk);
    }
}

// ---- JDK 8 PrintGCDetails (feat-069). Counts from grep over record-start lines. ----

const J8_PARALLEL: &str = include_str!("../../tests/fixtures/gc/jdk8-parallel.log");
const J8_PARALLEL_REFGC: &str = include_str!("../../tests/fixtures/gc/jdk8-parallel-refgc.log");
const J8_PARALLEL_NOSTAMPS: &str = include_str!("../../tests/fixtures/gc/jdk8-parallel-nostamps.log");
const J8_G1: &str = include_str!("../../tests/fixtures/gc/jdk8-g1.log");
const J8_CMS: &str = include_str!("../../tests/fixtures/gc/jdk8-cms.log");
const J8_SERIAL: &str = include_str!("../../tests/fixtures/gc/jdk8-serial.log");

#[test]
fn gc_legacy_detects_format() {
    for text in [J8_PARALLEL, J8_PARALLEL_REFGC, J8_PARALLEL_NOSTAMPS, J8_G1, J8_CMS, J8_SERIAL] {
        assert_eq!(detect_format(text), GcLogFormat::Legacy);
    }
}

#[test]
fn gc_legacy_parallel_events_and_header() {
    let log = parse_log(J8_PARALLEL);
    assert_eq!(log.format, GcLogFormat::Legacy);
    assert_eq!(log.collector, Collector::Parallel);
    assert_eq!(log.events.len(), 61);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 4);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    assert_eq!(log.jvm.version.as_deref(), Some("1.8.0_512-b09"));
    assert_eq!(log.jvm.heap_max_kb, Some(64.0 * 1024.0));
    assert!(log.jvm.command_line.as_deref().unwrap().contains("-XX:+UseParallelGC"));
    // First record: [GC (Allocation Failure) [PSYoungGen: 16384K->…] 16384K->…(62976K), 0.000… secs]
    let e = &log.events[0];
    assert_eq!((e.kind, e.name.as_str(), e.cause.as_deref()), (GcKind::Young, "Pause Young", Some("Allocation Failure")));
    assert_eq!(e.young.unwrap().before_kb, 16384.0);
    assert_eq!(e.heap.unwrap().before_kb, 16384.0);
    assert_eq!(e.heap.unwrap().capacity_after_kb, Some(62976.0));
    assert!(e.duration_ms > 0.0 && e.duration_ms < 10.0);
    assert!(e.cpu.is_some() && e.uptime_ms.is_some() && e.wall_ms.is_some());
    let full = log.events.iter().find(|e| e.kind == GcKind::Full).unwrap();
    assert_eq!(full.cause.as_deref(), Some("System.gc()"));
    assert!(full.old.is_some() && full.metaspace.is_some() && full.young.is_some());
    assert_sorted(&log);
}

#[test]
fn gc_legacy_parallel_reference_gc_and_tenuring_multiline() {
    // PrintReferenceGC + PrintTenuringDistribution split each young GC over three lines (like real production logs).
    let log = parse_log(J8_PARALLEL_REFGC);
    assert_eq!(log.events.len(), 66);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 12);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    let young = log.events.iter().find(|e| e.kind == GcKind::Young).unwrap();
    assert!(young.young.is_some() && young.heap.is_some() && young.cpu.is_some());
    // The pause is the record total (`…, 0.0004925 secs]` on the third line), not a
    // `[SoftReference, …, 0.0000084 secs]` sub-phase from the first line.
    assert!((log.events[0].duration_ms - 0.4925).abs() < 1e-9, "{}", log.events[0].duration_ms);
    assert!(log.events.iter().all(|e| e.heap.is_some()));
}

#[test]
fn gc_legacy_without_stamps_uses_order() {
    // Strip the date/uptime stamps from a real log: events stay in order, timestamps are reported missing.
    let re = regex::Regex::new(r"(?m)(\d{4}-\d\d-\d\dT[\d:.]+[+-]\d{4}: )?\d+\.\d+: ").unwrap();
    let bare = re.replace_all(J8_PARALLEL_NOSTAMPS, "").into_owned();
    let log = parse_log(&bare);
    assert_eq!(log.events.len(), 19);
    assert!(log.missing_timestamps);
    assert_eq!(log.events[3].time_ms, 3.0);
    let stamped = parse_log(J8_PARALLEL_NOSTAMPS);
    assert!(!stamped.missing_timestamps && stamped.events.iter().all(|e| e.uptime_ms.is_some() && e.wall_ms.is_none()));
}

#[test]
fn gc_legacy_g1_pauses_concurrent_and_failures() {
    let log = parse_log(J8_G1);
    assert_eq!(log.collector, Collector::G1);
    // 78 `[GC pause` / remark / cleanup / Full GC records (grep excluding `[GC concurrent-…]` lines).
    assert_eq!(count(&log, |e| e.is_pause), 78);
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 34);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 4);
    assert_eq!(count(&log, |e| e.kind == GcKind::Mixed), 7);
    assert_eq!(count(&log, |e| e.kind == GcKind::Remark), 17);
    assert_eq!(count(&log, |e| e.kind == GcKind::Cleanup), 17);
    assert_eq!(count(&log, |e| e.flags.evacuation_failure), 4);
    assert_eq!(count(&log, |e| e.name == "Pause Young (Concurrent Start)"), 17);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    let y = log.events.iter().find(|e| e.name == "Pause Young (Normal)").unwrap();
    assert_eq!(y.cause.as_deref(), Some("G1 Evacuation Pause"));
    let (young, heap, old) = (y.young.unwrap(), y.heap.unwrap(), y.old.unwrap());
    assert!(young.before_kb > 0.0 && heap.capacity_after_kb.is_some());
    assert!((old.before_kb + young.before_kb - heap.before_kb).abs() < 1e-6);
    // Remark duration is the record total, not its Finalize Marking / ref-proc sub-phases.
    let remark = log.events.iter().find(|e| e.kind == GcKind::Remark).unwrap();
    assert!(remark.duration_ms > 0.0 && remark.cpu.is_some());
    assert_sorted(&log);
}

#[test]
fn gc_legacy_cms_phases_concurrent_and_promotion_failure() {
    let log = parse_log(J8_CMS);
    assert_eq!(log.collector, Collector::Cms);
    assert_eq!(count(&log, |e| e.is_pause), 250);
    assert_eq!(count(&log, |e| e.kind == GcKind::InitialMark), 74);
    assert_eq!(count(&log, |e| e.kind == GcKind::Remark), 74);
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 326);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    // [GC (Allocation Failure) [ParNew (promotion failed): …][CMS: 32166K->32765K(32768K), …] …] is a full collection.
    let pf: Vec<_> = log.events.iter().filter(|e| e.flags.promotion_failed).collect();
    assert_eq!(pf.len(), 1);
    assert_eq!(pf[0].kind, GcKind::Full);
    let old = pf[0].old.unwrap();
    assert_eq!((old.before_kb, old.after_kb, old.capacity_after_kb), (32166.0, 32765.0, Some(32768.0)));
    let im = log.events.iter().find(|e| e.kind == GcKind::InitialMark).unwrap();
    assert!(im.old.is_some() && im.heap.is_some());
    let young = log.events.iter().find(|e| e.kind == GcKind::Young).unwrap();
    assert!(young.young.is_some() && young.heap.is_some());
}

#[test]
fn gc_legacy_serial() {
    let log = parse_log(J8_SERIAL);
    assert_eq!(log.collector, Collector::Serial);
    assert_eq!(log.events.len(), 23);
    assert_eq!(count(&log, |e| e.kind == GcKind::Full), 3);
    assert!(log.events.iter().all(|e| e.heap.is_some()));
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
}

#[test]
fn gc_legacy_truncated_and_garbage_input_does_not_panic() {
    for text in [J8_CMS, J8_G1, J8_PARALLEL_REFGC] {
        for cut in [10, 500, 4000, text.len() / 3, text.len() - 7] {
            let mut end = cut.min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            let _ = parse_log(&text[..end]);
        }
    }
    let truncated = format!("{J8_PARALLEL}2026-11-13T11:19:53.829+0800: 334711.118: [Full GC (Ergonomics) \n");
    let log = parse_log(&truncated);
    assert_eq!(log.events.len(), 61);
    assert_eq!(log.unparsed_count, 1, "an unfinished trailing record is reported, not fatal");
}

// ---- ZGC / Shenandoah (feat-070). Counts from grep over the JDK 21 fixtures. ----

const ZGC: &str = include_str!("../../tests/fixtures/gc/zgc-jdk21.log");
const ZGC_GEN: &str = include_str!("../../tests/fixtures/gc/zgc-generational-jdk21.log");
const SHENANDOAH: &str = include_str!("../../tests/fixtures/gc/shenandoah-jdk21.log");
const SHENANDOAH_PASSIVE: &str = include_str!("../../tests/fixtures/gc/shenandoah-passive-jdk21.log");

#[test]
fn gc_zgc_pauses_cycles_and_stalls() {
    let log = parse_log(ZGC);
    assert_eq!(log.collector, Collector::Zgc);
    assert_eq!(log.events.len(), 146);
    assert_eq!(count(&log, |e| e.is_pause), 105, "Pause Mark Start / Mark End / Relocate Start on gc,phases");
    assert!(log.events.iter().filter(|e| e.is_pause).all(|e| e.kind == GcKind::OtherPause));
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 35);
    assert_eq!(count(&log, |e| e.kind == GcKind::Stall && e.flags.allocation_stall), 6);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    // `Garbage Collection (Allocation Stall) 42M(66%)->30M(47%)`: heap from percentages, capacity = max heap,
    // duration = summary time − gc,start time (non-generational ZGC prints no duration).
    let cycle = log.events.iter().find(|e| e.kind == GcKind::Concurrent).unwrap();
    assert_eq!(cycle.name, "Garbage Collection");
    assert!(cycle.cause.is_some());
    let heap = cycle.heap.unwrap();
    assert!(heap.before_kb > 0.0 && heap.capacity_after_kb == log.jvm.heap_max_kb);
    assert!(cycle.duration_ms > 0.0 && cycle.metaspace.is_some());
    let stall = log.events.iter().find(|e| e.kind == GcKind::Stall).unwrap();
    assert_eq!(stall.cause.as_deref(), Some("main"));
    // Heap KPIs come from cycles for concurrent collectors.
    let s = stats::compute(&log.events, None, log.jvm.heap_max_kb);
    assert!(s.heap_after_peak_pct.is_some() && s.allocation.is_some());
    assert_eq!(s.pause_count, 105);
}

#[test]
fn gc_zgc_generational_minor_major() {
    let log = parse_log(ZGC_GEN);
    assert_eq!(log.collector, Collector::Zgc);
    assert_eq!(log.events.len(), 310);
    assert_eq!(count(&log, |e| e.is_pause), 231, "y:/Y:/o: prefixed pauses");
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 76);
    assert_eq!(count(&log, |e| e.kind == GcKind::Stall), 3);
    assert!(count(&log, |e| e.name == "Minor Collection") > 0 && count(&log, |e| e.name == "Major Collection") > 0);
    // Generational ZGC prints the cycle duration (`… 0.002s`).
    let minor = log.events.iter().find(|e| e.name == "Minor Collection").unwrap();
    assert!(minor.duration_ms > 0.0 && minor.heap.is_some());
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
}

#[test]
fn gc_shenandoah_pauses_concurrent_and_degenerated() {
    let log = parse_log(SHENANDOAH);
    assert_eq!(log.collector, Collector::Shenandoah);
    assert_eq!(count(&log, |e| e.is_pause), 77);
    assert_eq!(count(&log, |e| e.kind == GcKind::Concurrent), 181);
    assert_eq!(count(&log, |e| e.flags.degenerated), 1);
    assert_eq!(log.unparsed_count, 0, "{:?}", log.unparsed_samples);
    // Qualifiers stay in the name, not in the cause.
    let init = log.events.iter().find(|e| e.name.starts_with("Pause Init Mark")).unwrap();
    assert_eq!(init.cause, None);
    // `Concurrent cleanup 8M->3M(32M)` carries the heap.
    let cleanup = log.events.iter().find(|e| e.name == "Concurrent cleanup").unwrap();
    assert!(cleanup.heap.is_some());

    let passive = parse_log(SHENANDOAH_PASSIVE);
    assert_eq!(count(&passive, |e| e.flags.degenerated && e.is_pause), 14);
    let d = &passive.events[0];
    assert_eq!(d.name, "Pause Degenerated GC (Outside of Cycle)");
    assert!(d.heap.is_some());
    let f = findings::evaluate(&passive, &stats::compute(&passive.events, None, passive.jvm.heap_max_kb));
    let g14 = f.iter().find(|x| x.rule == "GC14").unwrap();
    assert_eq!((g14.severity.as_str(), g14.detail.as_str()), ("warning", "stalls=0;stalls_per_min=0;degenerated=14"));
}
