"""Apply the phase-accounting instrumentation to a genson-core checkout (measurement only).

Adds a counting allocator (global and per-thread counts of allocations and requested bytes)
and, when the ACCT environment variable is set, prints one line per phase of
`infer_json_schema_from_strings` to stderr:

    ACCT <phase> wall=<s> cpu=<s> allocs=<n> bytes=<n>

and, for the per-string work of the parallel path, sums across threads:

    ACCT row.<stage> thread_s=<s> allocs=<n> bytes=<n>

Usage: python acct_patch.py <repo checkout> [--no-count]

With --no-count the allocator is left as it is (allocation fields read 0), so that the
timings carry no counting overhead: a shared atomic counter on every allocation roughly
triples wall time on 4 threads.
"""

import sys

root = sys.argv[1] + "/genson-core/src"
COUNT = "--no-count" not in sys.argv


def edit(path, old, new, count=1):
    with open(path) as f:
        s = f.read()
    assert s.count(old) == count, (path, old[:60], s.count(old))
    s = s.replace(old, new)
    with open(path, "w") as f:
        f.write(s)


# 1. Counting allocator, with per-thread counters for the per-string stages
NO_COUNT = """// MEASUREMENT ONLY: allocation counters left at 0 (--no-count)
pub static ALLOCS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ALLOC_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn thread_allocs() -> (u64, u64) {
    (0, 0)
}
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
"""
COUNTING = """// MEASUREMENT ONLY: mimalloc behind counters of allocations and requested bytes
pub static ALLOCS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ALLOC_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
thread_local! {
    pub static T_ALLOCS: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}
pub struct Counting;
#[inline]
fn count(bytes: usize) {
    use std::sync::atomic::Ordering::Relaxed;
    ALLOCS.fetch_add(1, Relaxed);
    ALLOC_BYTES.fetch_add(bytes as u64, Relaxed);
    let _ = T_ALLOCS.try_with(|c| {
        let (n, b) = c.get();
        c.set((n + 1, b + bytes as u64));
    });
}
pub fn thread_allocs() -> (u64, u64) {
    T_ALLOCS.try_with(|c| c.get()).unwrap_or((0, 0))
}
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        count(layout.size());
        MiMalloc.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: std::alloc::Layout) -> *mut u8 {
        count(layout.size());
        MiMalloc.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        count(new_size);
        MiMalloc.realloc(ptr, layout, new_size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        MiMalloc.dealloc(ptr, layout)
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;
"""
edit(
    f"{root}/genson_rs/mod.rs",
    """// Setting the global allocator to mimalloc for more efficient memory allocation
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
""",
    COUNTING if COUNT else NO_COUNT,
)
if COUNT:
    edit(
        f"{root}/genson_rs/mod.rs",
        "use mimalloc::MiMalloc;",
        "use mimalloc::MiMalloc;\nuse std::alloc::GlobalAlloc;",
    )

# 2. Phase timers
ACCT = """
/// MEASUREMENT ONLY: phase accounting printed to stderr when ACCT is set.
pub mod acct {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
    use std::time::Instant;
    pub static ROW: [[AtomicU64; 3]; 5] = [const { [const { AtomicU64::new(0) }; 3] }; 5];
    pub const ROW_NAMES: [&str; 5] = ["prep", "build", "force", "hash", "drop_dup"];
    pub fn on() -> bool {
        std::env::var_os("ACCT").is_some()
    }
    /// (user s, system s, minor page faults) of the whole process so far.
    fn usage() -> (f64, f64, f64) {
        #[repr(C)]
        struct Rusage {
            ru_utime: [i64; 2],
            ru_stime: [i64; 2],
            rest: [i64; 14],
        }
        extern "C" {
            fn getrusage(who: i32, usage: *mut Rusage) -> i32;
        }
        let mut ru: Rusage = unsafe { std::mem::zeroed() };
        unsafe { getrusage(0, &mut ru) };
        let s = |tv: [i64; 2]| tv[0] as f64 + tv[1] as f64 / 1e6;
        (s(ru.ru_utime), s(ru.ru_stime), ru.rest[4] as f64)
    }
    pub struct Phase(&'static str, Instant, (f64, f64, f64), u64, u64);
    impl Phase {
        pub fn start(name: &'static str) -> Self {
            let (a, b) = snap();
            Phase(name, Instant::now(), usage(), a, b)
        }
        pub fn end(self) {
            if on() {
                let (a, b) = snap();
                let (u, s, f) = usage();
                eprintln!(
                    "ACCT {} wall={:.4} user={:.4} sys={:.4} faults={} allocs={} bytes={}",
                    self.0,
                    self.1.elapsed().as_secs_f64(),
                    u - self.2 .0,
                    s - self.2 .1,
                    f - self.2 .2,
                    a - self.3,
                    b - self.4
                );
            }
        }
    }
    fn snap() -> (u64, u64) {
        (
            crate::genson_rs::ALLOCS.load(Relaxed),
            crate::genson_rs::ALLOC_BYTES.load(Relaxed),
        )
    }
    /// Time and count `f` on this thread into row stage `i`.
    pub fn row<T>(i: usize, f: impl FnOnce() -> T) -> T {
        let (a, b) = crate::genson_rs::thread_allocs();
        let t = Instant::now();
        let out = f();
        let (a2, b2) = crate::genson_rs::thread_allocs();
        ROW[i][0].fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
        ROW[i][1].fetch_add(a2 - a, Relaxed);
        ROW[i][2].fetch_add(b2 - b, Relaxed);
        out
    }
    pub fn print_rows() {
        if on() {
            for (i, name) in ROW_NAMES.iter().enumerate() {
                eprintln!(
                    "ACCT row.{} thread_s={:.4} allocs={} bytes={}",
                    name,
                    ROW[i][0].swap(0, Relaxed) as f64 / 1e9,
                    ROW[i][1].swap(0, Relaxed),
                    ROW[i][2].swap(0, Relaxed)
                );
            }
        }
    }
}
"""
edit(
    f"{root}/schema.rs",
    "/// Maximum length of JSON string",
    ACCT + "\n/// Maximum length of JSON string",
)

s = f"{root}/schema.rs"
edit(
    s,
    "let prepared = prepare_json_bytes(json_str.as_ref().as_bytes(), i, config)?;",
    "let prepared = acct::row(0, || prepare_json_bytes(json_str.as_ref().as_bytes(), i, config))?;",
)
edit(
    s,
    "build_json_schema_into(&mut chunk_builder, &mut bytes, &chunk_build_config);",
    "acct::row(1, || build_json_schema_into(&mut chunk_builder, &mut bytes, &chunk_build_config));\n                    acct::row(0, || drop(bytes));",
)
edit(
    s,
    """                    if forces_maps {
                        apply_force_field_types_to_node(chunk_builder.root_node_mut(), config);
                    }
                    let hash = chunk_builder.schema_hash();""",
    """                    if forces_maps {
                        acct::row(2, || apply_force_field_types_to_node(chunk_builder.root_node_mut(), config));
                    }
                    let hash = acct::row(3, || chunk_builder.schema_hash());""",
)
edit(
    s,
    "        let t_par = std::time::Instant::now();",
    '        let t_par = std::time::Instant::now();\n        let ph = acct::Phase::start("chunk.parallel");',
)
edit(
    s,
    "        let par_el = t_par.elapsed();",
    '        let par_el = t_par.elapsed();\n        ph.end();\n        let ph = acct::Phase::start("chunk.merge");',
)
edit(
    s,
    "        builder.add_builders(builders);\n",
    '        builder.add_builders(builders);\n        ph.end();\n        let ph = acct::Phase::start("chunk.drop_dups");\n',
)
edit(
    s,
    "        duplicates.into_par_iter().for_each(drop);\n",
    "        duplicates.into_par_iter().for_each(drop);\n        ph.end();\n",
)
edit(
    s,
    '    profile!(config, "All chunks processed ({})", current_time_hms());\n',
    '    profile!(config, "All chunks processed ({})", current_time_hms());\n    acct::print_rows();\n',
)
edit(
    s,
    "            let processed_count = if use_parallel {",
    '            let ph_all = acct::Phase::start("total");\n            let ph = acct::Phase::start("build_all");\n            let processed_count = if use_parallel {',
)
edit(
    s,
    "            // Get final schema\n            let mut final_schema = builder.to_schema();\n",
    '            ph.end();\n            let ph = acct::Phase::start("to_schema");\n            let mut final_schema = builder.to_schema();\n            ph.end();\n            let ph = acct::Phase::start("drop_builder");\n            drop(builder);\n            ph.end();\n            let ph = acct::Phase::start("preprocess_force");\n',
)
edit(
    s,
    "            rewrite_objects(&mut final_schema, None, &config, true);\n",
    '            ph.end();\n            let ph = acct::Phase::start("rewrite_objects");\n            rewrite_objects(&mut final_schema, None, &config, true);\n            ph.end();\n            let ph = acct::Phase::start("reorder_unions");\n',
)
edit(
    s,
    "            reorder_unions(&mut final_schema);\n\n",
    "            reorder_unions(&mut final_schema);\n            ph.end();\n            ph_all.end();\n\n",
)
print("patched", root)
