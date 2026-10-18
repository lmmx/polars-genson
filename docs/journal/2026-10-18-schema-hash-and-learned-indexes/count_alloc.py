"""Put a counter of allocations and requested bytes around mimalloc in a genson-core checkout.

Measurement only, for alloc_count.rs; not part of genson-core. Usage: python count_alloc.py <repo>
"""

import sys

p = sys.argv[1] + "/genson-core/src/genson_rs/mod.rs"
with open(p) as f:
    s = f.read()
old = """// Setting the global allocator to mimalloc for more efficient memory allocation
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
"""
new = """// MEASUREMENT ONLY: mimalloc behind a counter of allocations and requested bytes
pub static ALLOCS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static ALLOC_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub struct Counting;
fn count(bytes: usize) {
    use std::sync::atomic::Ordering::Relaxed;
    ALLOCS.fetch_add(1, Relaxed);
    ALLOC_BYTES.fetch_add(bytes as u64, Relaxed);
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
assert old in s
s = s.replace(old, new)
s = s.replace(
    "use mimalloc::MiMalloc;", "use mimalloc::MiMalloc;\nuse std::alloc::GlobalAlloc;"
)
with open(p, "w") as f:
    f.write(s)
