import sys
p=sys.argv[1]+"/genson-core/src/phase.rs"; s=open(p).read()
s=s.replace("pub use stats::{take_stats, Counting, StageAllocs};","pub use stats::{counters, take_stats, Counting, StageAllocs};")
s=s.replace("    pub(super) fn enter() -> (u64, u64) {","""    /// MEASUREMENT ONLY
    pub fn counters(reset: bool) -> (u64, u64, u64, u64) {
        if reset {
            PEAK.store(LIVE.load(Relaxed), Relaxed);
        }
        (ALLOCS.load(Relaxed), BYTES.load(Relaxed), LIVE.load(Relaxed), PEAK.load(Relaxed))
    }

    pub(super) fn enter() -> (u64, u64) {""",1)
open(p,"w").write(s)
