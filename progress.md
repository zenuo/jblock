# Session Progress Log

## Current State

**Last Updated:** 2026-07-27 09:30
**Active Feature:** feat-052 (done)

## Status

### What's Done

- [x] **feat-052** Legend hover tip includes thread id
  - `FindingActor.id` from `ThreadInfo.id`
  - `actorsForNames` skips already-used ThreadInfo when names collide
  - Tip shows `Id={id}` under the full thread name

### What's In Progress

- [ ] (none)

### What's Next

1. User feedback on denser results / legend tips

## Decisions Made

- Display as `Id=N` for both jstack `#N` and MXBean `Id=N` dumps (same numeric string)
- Duplicate-name peers get distinct ids by consuming analysis.threads in order

## Evidence of Completion

```text
$ ./init.sh
… Summary: 52/52 features PASS
```
