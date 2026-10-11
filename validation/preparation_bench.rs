//! Isolate full-data preparation: ROWS COLUMNS EMPTY_MODE MATCHER REPETITIONS.
//! EMPTY_MODE: none, all, first, last, mixed; MATCHER: noop, coma, cupid, jaccard.
//! Two tables; 35-byte text values, 128 distinct values per populated column.
//! One untimed warmup, then timed public valentine_match calls (including drop).
//! Generation, result hashing and printing are excluded. Jaccard uses Exact.
//! Compile normally for timing; --cfg 'feature="allocations"' enables the
//! counting allocator. Its extra peak is live Rust heap above the input baseline,
//! not RSS; zero allocation fields in normal builds are uninstrumented.
//! peak_rss_kib is this process's Linux VmHWM (null on other platforms).

use fieldkin::algorithms::{
    Coma, Cupid, JaccardConfig, JaccardDistanceMatcher, StringDistanceFunction,
};
use fieldkin::{DataType, Error, Field, Matcher, MatcherResults, Table, valentine_match};
#[cfg(feature = "allocations")]
use std::alloc::{GlobalAlloc, Layout, System};
use std::hash::{Hash, Hasher};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};
use std::time::Instant;
#[cfg(feature = "allocations")]
struct CountAlloc;
static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "allocations")]
unsafe impl GlobalAlloc for CountAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let live = LIVE.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(live, Relaxed);
            if COUNT.load(Relaxed) {
                ALLOCS.fetch_add(1, Relaxed);
                BYTES.fetch_add(l.size(), Relaxed);
            }
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            let live = if n >= l.size() {
                LIVE.fetch_add(n - l.size(), Relaxed) + n - l.size()
            } else {
                LIVE.fetch_sub(l.size() - n, Relaxed) - l.size() + n
            };
            PEAK.fetch_max(live, Relaxed);
            if COUNT.load(Relaxed) {
                ALLOCS.fetch_add(1, Relaxed);
                BYTES.fetch_add(n, Relaxed);
            }
        }
        q
    }
}
#[cfg(feature = "allocations")]
#[global_allocator]
static ALLOC: CountAlloc = CountAlloc;
struct Noop;
impl Matcher for Noop {
    fn get_matches(&self, _: &Table, _: &Table) -> Result<MatcherResults, Error> {
        unreachable!()
    }
    fn get_matches_batch(&self, t: &[Table]) -> Result<MatcherResults, Error> {
        black_box(t);
        MatcherResults::new(Vec::new())
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let rows: usize = args[1].parse().unwrap();
    let cols: usize = args[2].parse().unwrap();
    let empty = &args[3];
    let mode = &args[4];
    let repeats: usize = args[5].parse().unwrap();
    assert!(["none", "all", "first", "last", "mixed"].contains(&empty.as_str()));
    let tables: Vec<_> = (0..2)
        .map(|t| {
            Table::new(
                format!("table{t}"),
                (0..cols)
                    .map(|c| Field {
                        name: format!("field{c}"),
                        data_type: DataType::Text,
                        samples: (0..rows)
                            .map(|r| {
                                if empty == "all"
                                    || (empty == "first" && r == 0)
                                    || (empty == "last" && r + 1 == rows && c + 1 == cols && t == 1)
                                    || (empty == "mixed" && r % 2 == 0)
                                {
                                    String::new()
                                } else {
                                    format!("value{c:04}_{:08}_abcdefghijklmnop", r % 128)
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let noop = Noop;
    let coma = Coma::default();
    let cupid = Cupid::default();
    let jaccard = JaccardDistanceMatcher::new(JaccardConfig {
        distance_fun: StringDistanceFunction::Exact,
        ..Default::default()
    })
    .unwrap();
    let matcher: &dyn Matcher = match mode.as_str() {
        "coma" => &coma,
        "cupid" => &cupid,
        "jaccard" => &jaccard,
        "noop" => &noop,
        _ => panic!("unknown matcher"),
    };
    let before = LIVE.load(Relaxed);
    PEAK.store(before, Relaxed);
    ALLOCS.store(0, Relaxed);
    BYTES.store(0, Relaxed);
    COUNT.store(true, Relaxed);
    let result = valentine_match(&tables, matcher).unwrap();
    COUNT.store(false, Relaxed);
    let allocations = ALLOCS.load(Relaxed);
    let bytes = BYTES.load(Relaxed);
    let peak = PEAK.load(Relaxed) - before;
    let score_sum: f64 = result.iter().map(|(_, s)| *s).sum();
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for (p, s) in result.iter() {
        p.hash(&mut hash);
        s.to_bits().hash(&mut hash);
        if let Some(details) = result.get_details(p) {
            for (name, s) in details {
                name.hash(&mut hash);
                s.to_bits().hash(&mut hash);
            }
        }
    }
    let result_hash = hash.finish();
    drop(result);
    let mut times = Vec::new();
    for _ in 0..repeats {
        let now = Instant::now();
        black_box(valentine_match(black_box(&tables), matcher).unwrap());
        times.push(now.elapsed().as_nanos());
    }
    let peak_rss_kib = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")?
                    .split_whitespace()
                    .next()?
                    .parse::<usize>()
                    .ok()
            })
        })
        .map_or("null".to_owned(), |value| value.to_string());
    println!(
        "{{\"rows\":{rows},\"columns\":{cols},\"empties\":\"{empty}\",\"matcher\":\"{mode}\",\"allocations\":{allocations},\"allocated_bytes\":{bytes},\"extra_peak_heap_bytes\":{peak},\"score_sum\":{score_sum},\"result_hash\":{result_hash},\"times_ns\":{times:?},\"peak_rss_kib\":{peak_rss_kib}}}"
    );
}
