//! Inference's stages, each run in a function of its own that is never inlined, so that a
//! profiler attributes each stage's instructions by name whatever is inlined within it: the
//! performance scorecard (`scripts/scorecard.py`) reads the inclusive cost of each
//! `genson_core::phase::<stage>` from a callgrind run. A stage only runs its closure.
//!
//! With the `alloc-stats` feature the global allocator counts allocations, and each stage
//! records the allocations, bytes and peak live bytes it saw (`take_stats`). Without it the
//! stages cost one call each.

macro_rules! stages {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        $(
            $(#[$doc])*
            #[inline(never)]
            // Some stages run only under a feature (`avro`)
            #[allow(dead_code)]
            pub(crate) fn $name<R>(f: impl FnOnce() -> R) -> R {
                #[cfg(feature = "alloc-stats")]
                let start = stats::enter();
                let result = f();
                #[cfg(feature = "alloc-stats")]
                stats::leave(stringify!($name), start);
                result
            }
        )*
        /// The stages, in the order inference runs them.
        pub const STAGES: &[&str] = &[$(stringify!($name)),*];
    };
}

stages! {
    /// Reading the rows: their interned states, or (on the usual paths, which merge as
    /// they go) each row's schema merged into its chunk's builder and the chunks' builders
    /// into one
    rows,
    /// Merging the rows' interned states into the builder
    merge,
    /// Writing the builder's schema out: as a DAG, or (when debugging) as `Value`s
    write_schema,
    /// Map inference and unification, and fields forced to be maps
    rewrite,
    /// The rewritten DAG as a `Value`
    materialise,
    /// Union types put in their canonical order
    reorder,
    /// The conversion to Avro
    avro,
    /// Normalising rows to a schema (`normalise_values`), outside inference
    normalise,
}

#[cfg(feature = "alloc-stats")]
pub use stats::{take_stats, Counting, StageAllocs};

#[cfg(feature = "alloc-stats")]
mod stats {
    use std::alloc::{GlobalAlloc, Layout};
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    use std::sync::Mutex;

    static ALLOCS: AtomicU64 = AtomicU64::new(0);
    static BYTES: AtomicU64 = AtomicU64::new(0);
    static LIVE: AtomicU64 = AtomicU64::new(0);
    static PEAK: AtomicU64 = AtomicU64::new(0);
    static STAGES: Mutex<Vec<StageAllocs>> = Mutex::new(Vec::new());

    /// The allocations a stage made, the bytes they asked for, and the most bytes live
    /// at once while it ran.
    #[derive(Clone, Debug)]
    pub struct StageAllocs {
        pub stage: &'static str,
        pub allocations: u64,
        pub bytes: u64,
        pub peak_live: u64,
    }

    /// The stages recorded since the last call, in the order they ran.
    pub fn take_stats() -> Vec<StageAllocs> {
        std::mem::take(&mut *STAGES.lock().unwrap())
    }

    pub(super) fn enter() -> (u64, u64) {
        PEAK.store(LIVE.load(Relaxed), Relaxed);
        (ALLOCS.load(Relaxed), BYTES.load(Relaxed))
    }

    pub(super) fn leave(stage: &'static str, (allocations, bytes): (u64, u64)) {
        STAGES.lock().unwrap().push(StageAllocs {
            stage,
            allocations: ALLOCS.load(Relaxed) - allocations,
            bytes: BYTES.load(Relaxed) - bytes,
            peak_live: PEAK.load(Relaxed),
        });
    }

    fn grew(size: usize) {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(size as u64, Relaxed);
        let live = LIVE.fetch_add(size as u64, Relaxed) + size as u64;
        PEAK.fetch_max(live, Relaxed);
    }

    /// mimalloc, counting.
    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            grew(layout.size());
            unsafe { mimalloc::MiMalloc.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            LIVE.fetch_sub(layout.size() as u64, Relaxed);
            unsafe { mimalloc::MiMalloc.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            LIVE.fetch_sub(layout.size() as u64, Relaxed);
            grew(new_size);
            unsafe { mimalloc::MiMalloc.realloc(ptr, layout, new_size) }
        }
    }
}
