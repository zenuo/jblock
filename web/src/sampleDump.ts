import sampleRaw from "./sample.tdump?raw";

/**
 * Demo jstack dump for "Load sample".
 * Exercises findings (deadlock + hot lock), aggregated contention,
 * stack clusters, mixed states, stack frames, and JVM-noise filtering.
 * Source of truth: `sample.tdump` (also covered by a Rust parser test).
 */
export const SAMPLE_DUMP = sampleRaw;
