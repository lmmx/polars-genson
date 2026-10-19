//! The performance scorecard's workload (`scripts/scorecard.py`): schema inference on one of
//! a few fixed corpora, generated here from fixed seeds so that every revision is measured
//! on the same input without any data in the repository. Prints one JSON object: the
//! corpus's size, each run's wall and CPU time, the processed count, a hash of the schema,
//! peak RSS and (with the `alloc-stats` feature) each stage's allocations.
//!
//! Only `infer_json_schema_from_strings` and its config are used, so that the scorecard can
//! measure revisions from before it existed. `RAYON_NUM_THREADS` sets the threads.
//!
//! Each run's work is in `measured`, a function never inlined, which the callgrind runs
//! collect from.
//!
//! Usage: `scorecard <corpus> [--runs N] [--schema-out PATH]`, `scorecard --list`
use genson_core::{infer_json_schema_from_strings, SchemaInferenceConfig};
use std::fmt::Write as _;

/// A small deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }

    fn chance(&mut self, n: u64) -> bool {
        self.below(n) == 0
    }

    fn one<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }

    /// A number in `0..n`, small ones far likelier (roughly Zipf), as property and language
    /// popularity is.
    fn popular(&mut self, n: u64) -> u64 {
        let r = self.below(1 << 20) as f64 / (1 << 20) as f64;
        ((n as f64).powf(r) - 1.0) as u64
    }
}

const LANGUAGES: [&str; 40] = [
    "en", "fr", "de", "es", "it", "nl", "pl", "ru", "ja", "zh", "pt", "sv", "uk", "ca", "cs", "fi",
    "hu", "ko", "ar", "he", "id", "ro", "tr", "vi", "fa", "da", "nb", "el", "bg", "sr", "en-gb",
    "zh-hant", "zh-hans", "pt-br", "eo", "la", "cy", "ga", "eu", "gl",
];

/// A snak's value, of the datatype its property holds: one per property, as in Wikidata,
/// so that values differ in kind across properties but not within one.
fn datavalue(rng: &mut Rng, property: u64, out: &mut String) {
    let kind = property % 7;
    let (value, t) = match kind {
        0 | 1 => (format!("\"v{}\"", rng.below(100_000)), "string"),
        2 => {
            let q = rng.below(1_000_000);
            (
                format!("{{\"entity-type\":\"item\",\"numeric-id\":{q},\"id\":\"Q{q}\"}}"),
                "wikibase-entityid",
            )
        }
        3 => (
            format!(
                "{{\"time\":\"+{}-01-01T00:00:00Z\",\"timezone\":0,\"before\":0,\"after\":0,\"precision\":{},\"calendarmodel\":\"http://www.wikidata.org/entity/Q1985727\"}}",
                1800 + rng.below(220),
                9 + rng.below(3)
            ),
            "time",
        ),
        4 => {
            let bounds = if rng.chance(4) { ",\"upperBound\":\"+2\",\"lowerBound\":\"+1\"" } else { "" };
            (format!("{{\"amount\":\"+{}\",\"unit\":\"1\"{bounds}}}", rng.below(10_000)), "quantity")
        }
        5 => (
            format!("{{\"text\":\"t{}\",\"language\":\"{}\"}}", rng.below(1000), rng.one(&LANGUAGES)),
            "monolingualtext",
        ),
        _ => (
            format!(
                "{{\"latitude\":{}.5,\"longitude\":{}.25,\"altitude\":null,\"precision\":0.0001,\"globe\":\"http://www.wikidata.org/entity/Q2\"}}",
                rng.below(90),
                rng.below(180)
            ),
            "globecoordinate",
        ),
    };
    let _ = write!(out, "{{\"value\":{value},\"type\":\"{t}\"}}");
}

fn snak(rng: &mut Rng, property: u64, out: &mut String) {
    let snaktype = match rng.below(40) {
        0 => "somevalue",
        1 => "novalue",
        _ => "value",
    };
    let _ = write!(
        out,
        "{{\"snaktype\":\"{snaktype}\",\"property\":\"P{property}\",\"hash\":\"{:016x}\"",
        rng.below(u64::MAX)
    );
    if snaktype == "value" {
        out.push_str(",\"datavalue\":");
        datavalue(rng, property, out);
    }
    let _ = write!(out, ",\"datatype\":\"t{}\"}}", property % 7);
}

/// Snaks of a few properties, keyed by property, and their order.
fn snaks(rng: &mut Rng, out: &mut String) -> Vec<u64> {
    let mut properties: Vec<u64> = Vec::new();
    for _ in 0..1 + rng.below(3) {
        let p = rng.popular(400);
        if !properties.contains(&p) {
            properties.push(p);
        }
    }
    out.push('{');
    for (i, &p) in properties.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"P{p}\":[");
        snak(rng, p, out);
        out.push(']');
    }
    out.push('}');
    properties
}

fn order(properties: &[u64]) -> String {
    let names: Vec<String> = properties.iter().map(|p| format!("\"P{p}\"")).collect();
    format!("[{}]", names.join(","))
}

fn statement(rng: &mut Rng, property: u64, out: &mut String) {
    out.push_str("{\"mainsnak\":");
    snak(rng, property, out);
    let rank = match rng.below(20) {
        0 => "preferred",
        1 => "deprecated",
        _ => "normal",
    };
    let _ = write!(
        out,
        ",\"type\":\"statement\",\"id\":\"Q{}${:08x}\",\"rank\":\"{rank}\"",
        rng.below(1_000_000),
        rng.below(1 << 32)
    );
    if rng.chance(3) {
        out.push_str(",\"qualifiers\":");
        let qualifiers = snaks(rng, out);
        let _ = write!(out, ",\"qualifiers-order\":{}", order(&qualifiers));
    }
    if rng.chance(2) {
        let _ = write!(
            out,
            ",\"references\":[{{\"hash\":\"{:016x}\",\"snaks\":",
            rng.below(u64::MAX)
        );
        let references = snaks(rng, out);
        let _ = write!(out, ",\"snaks-order\":{}}}]", order(&references));
    }
    out.push('}');
}

/// An entity's claims: an object of properties' statements.
fn claims(rng: &mut Rng, properties: u64, out: &mut String) {
    let mut seen: Vec<u64> = Vec::new();
    out.push('{');
    for _ in 0..properties {
        let p = rng.popular(3000);
        if seen.contains(&p) {
            continue;
        }
        if !seen.is_empty() {
            out.push(',');
        }
        seen.push(p);
        let _ = write!(out, "\"P{p}\":[");
        for i in 0..1 + rng.popular(4) {
            if i > 0 {
                out.push(',');
            }
            statement(rng, p, out);
        }
        out.push(']');
    }
    out.push('}');
}

/// An object of languages' label records (`{"language": .., "value": ..}`).
fn label_map(rng: &mut Rng, languages: u64, out: &mut String) {
    let mut seen: Vec<u64> = Vec::new();
    out.push('{');
    for _ in 0..languages {
        let l = rng.popular(LANGUAGES.len() as u64 * 8);
        if seen.contains(&l) {
            continue;
        }
        if !seen.is_empty() {
            out.push(',');
        }
        seen.push(l);
        let code = match LANGUAGES.get(l as usize) {
            Some(code) => code.to_string(),
            None => format!("x{l}"),
        };
        let _ = write!(
            out,
            "\"{code}\":{{\"language\":\"{code}\",\"value\":\"n{}\"}}",
            rng.below(1000)
        );
    }
    out.push('}');
}

fn event(rng: &mut Rng, i: u64, out: &mut String) {
    let kind = rng.one(&[
        "PushEvent",
        "IssuesEvent",
        "PullRequestEvent",
        "WatchEvent",
        "ForkEvent",
        "IssueCommentEvent",
        "CreateEvent",
    ]);
    let user = |rng: &mut Rng| {
        let id = rng.below(10_000_000);
        let extra = if rng.chance(5) {
            ",\"display_name\":\"d\""
        } else {
            ""
        };
        format!(
            "{{\"id\":{id},\"login\":\"user{}\",\"site_admin\":{}{extra}}}",
            id % 5000,
            rng.chance(50)
        )
    };
    let actor = user(rng);
    let _ = write!(out, "{{\"id\":\"{}\",\"type\":\"{kind}\",\"actor\":{actor},\"repo\":{{\"id\":{},\"name\":\"org{}/repo{}\"}},\"payload\":", 10_000_000_000u64 + i, rng.below(100_000_000), rng.below(500), rng.below(500));
    match kind {
        "PushEvent" => {
            let commits: Vec<String> = (0..rng.below(4))
                .map(|_| format!("{{\"sha\":\"{:016x}\",\"author\":{{\"name\":\"a\",\"email\":\"e\"}},\"message\":\"m\",\"distinct\":{}}}", rng.below(u64::MAX), rng.chance(9)))
                .collect();
            let _ = write!(
                out,
                "{{\"push_id\":{},\"size\":{},\"ref\":\"refs/heads/main\",\"commits\":[{}]}}",
                rng.below(1 << 40),
                commits.len(),
                commits.join(",")
            );
        }
        "IssuesEvent" | "IssueCommentEvent" | "PullRequestEvent" => {
            let labels: Vec<String> = (0..rng.below(3))
                .map(|_| format!("{{\"name\":\"l{}\",\"color\":\"c\"}}", rng.below(20)))
                .collect();
            let closed = if rng.chance(2) {
                "\"2026-01-02T03:04:05Z\""
            } else {
                "null"
            };
            let issue = format!(
                "{{\"number\":{},\"title\":\"t\",\"state\":\"{}\",\"labels\":[{}],\"user\":{},\"comments\":{},\"closed_at\":{closed}}}",
                rng.below(10_000), rng.one(&["open", "closed"]), labels.join(","), user(rng), rng.below(50)
            );
            let key = if kind == "PullRequestEvent" {
                "pull_request"
            } else {
                "issue"
            };
            let comment = if kind == "IssueCommentEvent" {
                format!(
                    ",\"comment\":{{\"id\":{},\"body\":\"b\",\"user\":{}}}",
                    rng.below(1 << 30),
                    user(rng)
                )
            } else {
                String::new()
            };
            let _ = write!(
                out,
                "{{\"action\":\"{}\",\"{key}\":{issue}{comment}}}",
                rng.one(&["opened", "closed", "reopened"])
            );
        }
        "ForkEvent" => {
            let _ = write!(
                out,
                "{{\"forkee\":{{\"id\":{},\"full_name\":\"f\",\"public\":true}}}}",
                rng.below(1 << 30)
            );
        }
        "CreateEvent" => {
            let _ = write!(
                out,
                "{{\"ref\":\"v{}\",\"ref_type\":\"{}\",\"master_branch\":\"main\"}}",
                rng.below(9),
                rng.one(&["tag", "branch"])
            );
        }
        _ => out.push_str("{\"action\":\"started\"}"),
    }
    let org = if rng.chance(3) {
        format!(
            ",\"org\":{{\"id\":{},\"login\":\"org{}\"}}",
            rng.below(1 << 20),
            rng.below(500)
        )
    } else {
        String::new()
    };
    let _ = write!(
        out,
        ",\"public\":{},\"created_at\":\"2026-03-10T13:00:00Z\"{org}}}",
        !rng.chance(30)
    );
}

fn narrow(rng: &mut Rng, i: u64, out: &mut String) {
    let score = if rng.chance(3) {
        format!("{}.5", rng.below(9))
    } else {
        rng.below(9).to_string()
    };
    let tags: Vec<&str> = (0..rng.below(4))
        .map(|_| rng.one(&["\"a\"", "\"b\"", "\"c\"", "\"d\""]))
        .collect();
    let extra = if rng.chance(5) { ",\"extra\":null" } else { "" };
    let _ = write!(
        out,
        "{{\"id\":{i},\"name\":\"user{i}\",\"active\":{},\"score\":{score},\"tags\":[{}],\"meta\":{{\"source\":\"{}\",\"v\":{}{extra}}}}}",
        rng.chance(2), tags.join(","), rng.one(&["web", "app"]), 1 + rng.below(3)
    );
}

/// An entity as Wikidata's JSON dumps write it, much reduced.
fn entity(rng: &mut Rng, i: u64, out: &mut String) {
    let _ = write!(out, "{{\"type\":\"item\",\"id\":\"Q{i}\",\"labels\":");
    {
        let n = 5 + rng.below(60);
        label_map(rng, n, out)
    };
    out.push_str(",\"descriptions\":");
    {
        let n = rng.below(30);
        label_map(rng, n, out)
    };
    out.push_str(",\"claims\":");
    {
        let n = 3 + rng.below(40);
        claims(rng, n, out)
    };
    out.push_str(",\"sitelinks\":{");
    for j in 0..rng.below(20) {
        if j > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "\"w{j}wiki\":{{\"site\":\"w{j}wiki\",\"title\":\"t\",\"badges\":[]}}"
        );
    }
    out.push_str("}}");
}

/// A corpus: its rows, and the config inference runs with.
fn corpus(name: &str) -> Option<(Vec<String>, SchemaInferenceConfig)> {
    // The usual path's corpus is the event logs with a field forced to be a map, which the
    // interned path leaves to it; normalising's is the claims
    let name = match name {
        "usual" => {
            let (rows, config) = corpus("events")?;
            let forced = [("payload".to_string(), "map".to_string())].into();
            return Some((
                rows,
                SchemaInferenceConfig {
                    force_field_types: forced,
                    ..config
                },
            ));
        }
        "normalise" => "claims",
        name => name,
    };
    let mut rng = Rng(0x2545_f491_4f6c_dd1d ^ name.len() as u64);
    let rows = |n: u64, rng: &mut Rng, row: &mut dyn FnMut(&mut Rng, u64, &mut String)| {
        (0..n)
            .map(|i| {
                let mut out = String::new();
                row(rng, i, &mut out);
                out
            })
            .collect::<Vec<String>>()
    };
    let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect();
    #[allow(unused_mut)]
    let (rows, mut config) = match name {
        // Claims under wikidata-pq's config: map inference and unification of many properties
        "claims" => (
            rows(2000, &mut rng, &mut |rng, _, out| {
                let n = 1 + rng.popular(30);
                claims(rng, n, out)
            }),
            SchemaInferenceConfig {
                delimiter: Some(b'\n'),
                map_threshold: 0,
                unify_maps: true,
                force_field_types: [("mainsnak".to_string(), "record".to_string())].into(),
                force_scalar_promotion: set(&[
                    "mainsnak",
                    "datavalue",
                    "precision",
                    "latitude",
                    "longitude",
                ]),
                no_unify: set(&["qualifiers"]),
                wrap_root: Some("claims".to_string()),
                max_builders: Some(100),
                ..Default::default()
            },
        ),
        // Records of a few kinds, the defaults
        "events" => (
            rows(20_000, &mut rng, &mut event),
            SchemaInferenceConfig::default(),
        ),
        // Many rows of one small record
        "narrow" => (
            rows(100_000, &mut rng, &mut narrow),
            SchemaInferenceConfig::default(),
        ),
        // Maps of languages' labels, which become a map of records
        "labels" => (
            rows(1000, &mut rng, &mut |rng, _, out| {
                let n = 20 + rng.below(250);
                label_map(rng, n, out)
            }),
            SchemaInferenceConfig::default(),
        ),
        // Wide, varied entities (no Avro: its conversion of their schema is avrotize's cost)
        "entities" => (
            rows(150, &mut rng, &mut entity),
            SchemaInferenceConfig::default(),
        ),
        _ => return None,
    };
    #[cfg(feature = "avro")]
    {
        config.avro = name != "entities";
    }
    Some((rows, config))
}

const CORPORA: [&str; 7] = [
    "claims",
    "events",
    "narrow",
    "labels",
    "entities",
    "usual",
    "normalise",
];

/// What a run measures: inference, or normalising the rows to their inferred Avro schema.
/// It runs on the rayon pool, so that with one thread (as callgrind runs it) all the work
/// is in this call on one thread: callgrind collects a function's cost on the thread that
/// calls it.
fn measured<R: Send>(f: impl FnOnce() -> R + Send) -> R {
    #[inline(never)]
    fn on_the_pool<R>(f: impl FnOnce() -> R) -> R {
        f()
    }
    rayon::ThreadPoolBuilder::new()
        .build()
        .unwrap()
        .install(|| on_the_pool(f))
}

/// User and system CPU seconds, and peak RSS in KB (`getrusage`).
fn rusage() -> (f64, i64) {
    let mut ru: [i64; 18] = [0; 18];
    extern "C" {
        fn getrusage(who: i32, usage: *mut [i64; 18]) -> i32;
    }
    unsafe { getrusage(0, &mut ru) };
    ((ru[0] + ru[2]) as f64 + (ru[1] + ru[3]) as f64 / 1e6, ru[4])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--list") {
        println!("{}", CORPORA.join("\n"));
        return;
    }
    let name = args
        .get(1)
        .expect("usage: scorecard <corpus> [--runs N] [--schema-out PATH]");
    let flag = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
    };
    let runs: usize = flag("--runs").map_or(1, |n| n.parse().unwrap());
    let Some((rows, config)) = corpus(name) else {
        panic!("no corpus {name:?}: one of {CORPORA:?}");
    };
    let bytes: usize = rows.iter().map(String::len).sum();
    let mut timings = Vec::new();
    let mut last = None;
    // Normalising: the rows' inferred schema, and the rows read, before the runs
    #[cfg(feature = "avro")]
    let normalising = (name == "normalise").then(|| {
        let schema = infer_json_schema_from_strings(&rows, config.clone())
            .unwrap()
            .schema;
        let values: Vec<serde_json::Value> = rows
            .iter()
            .map(|r| serde_json::from_str(r).unwrap())
            .collect();
        (schema, values)
    });
    // Only the runs' stages are counted
    #[cfg(feature = "alloc-stats")]
    genson_core::phase::take_stats();
    for _ in 0..runs {
        #[cfg(feature = "avro")]
        if let Some((schema, values)) = &normalising {
            let cfg = genson_core::normalise::NormaliseConfig {
                map_encoding: genson_core::normalise::MapEncoding::KeyValueEntries,
                wrap_root: config.wrap_root.clone(),
                ..Default::default()
            };
            let values = values.clone();
            let (cpu, _) = rusage();
            let t = std::time::Instant::now();
            let out = measured(|| genson_core::normalise::normalise_values(values, schema, &cfg));
            let wall = t.elapsed().as_secs_f64();
            let rows: usize = out.len();
            timings.push(format!(
                "{{\"wall\":{wall:.6},\"cpu\":{:.6}}}",
                rusage().0 - cpu
            ));
            let normalised = serde_json::Value::Array(out).to_string();
            last = Some((normalised, rows));
            continue;
        }
        let (cpu, _) = rusage();
        let t = std::time::Instant::now();
        let result = measured(|| infer_json_schema_from_strings(&rows, config.clone())).unwrap();
        let wall = t.elapsed().as_secs_f64();
        let cpu = rusage().0 - cpu;
        timings.push(format!("{{\"wall\":{wall:.6},\"cpu\":{cpu:.6}}}"));
        last = Some((result.schema.to_string(), result.processed_count));
    }
    let (schema, processed) = last.expect("at least one run");
    if let Some(path) = flag("--schema-out") {
        std::fs::write(path, &schema).unwrap();
    }
    // The last run's stages, with `alloc-stats`
    #[cfg(feature = "alloc-stats")]
    let stages = {
        let recorded = genson_core::phase::take_stats();
        let from = recorded.len() - recorded.len() / runs.max(1);
        let entries: Vec<String> = recorded[from..]
            .iter()
            .map(|s| {
                format!(
                    "{{\"stage\":\"{}\",\"allocations\":{},\"bytes\":{},\"peak_live\":{}}}",
                    s.stage, s.allocations, s.bytes, s.peak_live
                )
            })
            .collect();
        format!(",\"stages\":[{}]", entries.join(","))
    };
    #[cfg(not(feature = "alloc-stats"))]
    let stages = "";
    println!(
        "{{\"corpus\":\"{name}\",\"rows\":{},\"bytes\":{bytes},\"threads\":{},\"processed\":{},\"schema_hash\":\"{:016x}\",\"peak_rss_kb\":{},\"runs\":[{}]{stages}}}",
        rows.len(),
        rayon::current_num_threads(),
        processed,
        xxhash_rust::xxh64::xxh64(schema.as_bytes(), 0),
        rusage().1,
        timings.join(","),
    );
}
