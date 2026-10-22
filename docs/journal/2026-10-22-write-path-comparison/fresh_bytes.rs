//! SCRATCH calibration: what writing bytes into fresh allocations costs on this machine with
//! genson's allocator (mimalloc, default settings), against writing them into memory already
//! touched. 4 threads, each copying `MB` megabytes as `rows` strings (extraction's slim rows),
//! or growing a Vec by push (dictionary indices, builders). Prints wall and minor faults.
use mimalloc::MiMalloc;
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use rayon::prelude::*;
use std::time::Instant;
fn minflt() -> u64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap();
    s.rsplit(") ").next().unwrap().split(' ').nth(7).unwrap().parse().unwrap()
}
fn run(name: &str, f: impl Fn(usize) + Sync) {
    let (t, m) = (Instant::now(), minflt());
    (0..4).into_par_iter().for_each(|i| f(i));
    println!("{name:48} {:7.3} s  {:8} faults", t.elapsed().as_secs_f64(), minflt() - m);
}
fn main() {
    let mb: usize = std::env::args().nth(1).map_or(300, |s| s.parse().unwrap());
    let per = mb * 1_000_000 / 4;
    let row = 37_000; // c0's mean claims row
    let src = vec![b'x'; row];
    let src = std::str::from_utf8(&src).unwrap();
    rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap();
    for round in 0..2 {
        println!("-- round {round}, {mb} MB in all");
        run("rows: a fresh String per row (kept)", |_| {
            let mut v = Vec::new();
            for _ in 0..per / row { let mut s = String::with_capacity(row); s.push_str(src); v.push(s); }
            std::hint::black_box(&v);
        });
        run("rows: one buffer per thread, reused per row", |_| {
            let mut s = String::with_capacity(row);
            for _ in 0..per / row { s.clear(); s.push_str(src); std::hint::black_box(&s); }
        });
        run("rows: one exact arena per thread (kept)", |_| {
            let mut s = String::with_capacity(per);
            for _ in 0..per / row { s.push_str(src); }
            std::hint::black_box(&s);
        });
        run("Vec<u64> grown by push to MB/8 values", |_| {
            let mut v: Vec<u64> = Vec::new(); for i in 0..per / 8 { v.push(i as u64); } std::hint::black_box(&v);
        });
        run("Vec<u32> grown by push, same count", |_| {
            let mut v: Vec<u32> = Vec::new(); for i in 0..per / 8 { v.push(i as u32); } std::hint::black_box(&v);
        });
        run("Vec<u64> exact capacity, same count", |_| {
            let mut v: Vec<u64> = Vec::with_capacity(per / 8); for i in 0..per / 8 { v.push(i as u64); } std::hint::black_box(&v);
        });
    }
}
