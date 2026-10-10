//! Native Cupid linguistic/structural schema matching, with bundled WordNet 3.0
//! senses and Wu-Palmer similarity. Rust executes the complete matching pipeline;
//! Python and NLTK are not runtime dependencies.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::{Mutex, OnceLock};

use flate2::read::GzDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::Matcher;
use super::coma::english_stopword;
use super::strings::{StringDistanceFunction, similarity};
use crate::{ColumnPair, DataType, Error, MatcherResults, Table};

/// Copyright notice and redistribution terms for the bundled WordNet corpus.
pub const WORDNET_LICENSE: &str = include_str!("../../assets/WORDNET-LICENSE");

/// Cupid weights, confidence thresholds and structural reinforcement factors.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CupidConfig {
    pub leaf_w_struct: f64,
    pub w_struct: f64,
    pub th_accept: f64,
    pub th_high: f64,
    pub th_low: f64,
    pub c_inc: f64,
    pub c_dec: f64,
    pub th_ns: f64,
    /// Native linguistic matching workers. Uses threads in place of Python
    /// processes, with identical scores regardless of worker count.
    pub process_num: usize,
}

impl Default for CupidConfig {
    fn default() -> Self {
        Self {
            leaf_w_struct: 0.2,
            w_struct: 0.2,
            th_accept: 0.7,
            th_high: 0.6,
            th_low: 0.35,
            c_inc: 1.2,
            c_dec: 0.9,
            th_ns: 0.7,
            process_num: 1,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cupid {
    pub config: CupidConfig,
}

impl Cupid {
    pub fn new(config: CupidConfig) -> Result<Self, Error> {
        let matcher = Self { config };
        matcher.validate()?;
        Ok(matcher)
    }

    fn validate(&self) -> Result<(), Error> {
        let c = &self.config;
        for (name, value) in [
            ("leaf_w_struct", c.leaf_w_struct),
            ("w_struct", c.w_struct),
            ("th_accept", c.th_accept),
            ("th_high", c.th_high),
            ("th_low", c.th_low),
            ("th_ns", c.th_ns),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::InvalidConfig(format!("{name} must be in [0, 1]")));
            }
        }
        for (name, value) in [("c_inc", c.c_inc), ("c_dec", c.c_dec)] {
            if !value.is_finite() || value <= 0.0 {
                return Err(Error::InvalidConfig(format!(
                    "{name} must be finite and positive"
                )));
            }
        }
        if c.process_num == 0 {
            return Err(Error::InvalidConfig("process_num must be positive".into()));
        }
        Ok(())
    }

    fn linguistic_matrix(
        &self,
        source: &Table,
        target: &Table,
    ) -> Result<Vec<Vec<LeafScore>>, Error> {
        let source_tokens: Vec<_> = source.columns.iter().map(|f| normalize(&f.name)).collect();
        let target_tokens: Vec<_> = target.columns.iter().map(|f| normalize(&f.name)).collect();
        let cache = SemanticCache::default();
        let workers = self.config.process_num.min(source.columns.len());
        let compute_row = |i: usize| {
            source
                .columns
                .get(i)
                .map(|s| {
                    target
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(j, t)| {
                            let compatibility =
                                datatype_compatibility(&s.data_type, &t.data_type, &cache);
                            let linguistic = if compatibility > self.config.th_ns {
                                name_similarity(&source_tokens[i], &target_tokens[j], &cache)
                                    * compatibility
                            } else {
                                0.0
                            };
                            let structural = 0.5 * compatibility;
                            LeafScore {
                                linguistic,
                                structural,
                                weighted: weighted(
                                    structural,
                                    linguistic,
                                    self.config.leaf_w_struct,
                                ),
                                compatibility,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        if workers <= 1 {
            return Ok((0..source.columns.len()).map(compute_row).collect());
        }
        std::thread::scope(|scope| {
            let chunk_size = source.columns.len().div_ceil(workers);
            let handles: Vec<_> = (0..source.columns.len())
                .step_by(chunk_size)
                .map(|start| {
                    let compute_row = &compute_row;
                    std::thread::Builder::new().spawn_scoped(scope, move || {
                        (start..(start + chunk_size).min(source.columns.len()))
                            .map(compute_row)
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Result<_, _>>()?;
            let mut matrix = Vec::with_capacity(source.columns.len());
            for handle in handles {
                matrix.extend(
                    handle
                        .join()
                        .map_err(|_| Error::Algorithm("Cupid worker failed".into()))?,
                );
            }
            Ok(matrix)
        })
    }
}

impl Matcher for Cupid {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.validate()?;
        super::validate_tables([source, target])?;
        if source.columns.is_empty() || target.columns.is_empty() {
            return MatcherResults::new(Vec::new());
        }
        let mut matrix = self.linguistic_matrix(source, target)?;
        let c = &self.config;

        // Valentine constructs database -> table -> columns. Both inner nodes
        // have the same leaves. Postorder therefore visits exactly these four
        // pairs: table/table, table/database, database/table, database/database.
        // Only column datatypes enter its compatibility corpus, so inner-node
        // linguistic scores are zero. Re-evaluate links after each update:
        // earlier reinforcement can move a leaf across the acceptance boundary.
        for _ in 0..4 {
            if let Some(structural) = structural_similarity(&matrix, c.th_accept) {
                let inner_weighted = weighted(structural, 0.0, c.w_struct);
                if inner_weighted > c.th_high {
                    reinforce(&mut matrix, c.c_inc, c.leaf_w_struct);
                }
                if inner_weighted < c.th_low {
                    reinforce(&mut matrix, c.c_dec, c.leaf_w_struct);
                }
            }
        }

        // Upstream recompute_wsim subsequently recomputes *inner-node* scores
        // at weight .6 and acceptance .14. It never alters leaf scores, and
        // mapping_generation_leaves discards every inner node. No intermediate
        // inner-node allocation is needed for this exactly equivalent output.
        let mut entries = Vec::new();
        let mut details = BTreeMap::new();
        for (i, row) in matrix.iter().enumerate() {
            for (j, score) in row.iter().enumerate() {
                if score.weighted >= c.th_accept {
                    let pair = ColumnPair::new(
                        &source.name,
                        &source.columns[i].name,
                        &target.name,
                        &target.columns[j].name,
                    );
                    entries.push((pair.clone(), score.weighted));
                    details.insert(
                        pair,
                        BTreeMap::from([
                            ("lsim".into(), score.linguistic),
                            ("ssim".into(), score.structural),
                            ("wsim".into(), score.weighted),
                            ("datatype".into(), score.compatibility),
                        ]),
                    );
                }
            }
        }
        MatcherResults::with_details(entries, details)
    }
}

#[derive(Clone, Copy, Debug)]
struct LeafScore {
    linguistic: f64,
    structural: f64,
    weighted: f64,
    compatibility: f64,
}

fn weighted(structural: f64, linguistic: f64, weight: f64) -> f64 {
    weight * structural + (1.0 - weight) * linguistic
}

fn structural_similarity(matrix: &[Vec<LeafScore>], threshold: f64) -> Option<f64> {
    let m = matrix.len();
    let n = matrix.first().map_or(0, Vec::len);
    if m > n.saturating_mul(2) || n > m.saturating_mul(2) {
        return None;
    }
    if m + n == 0 {
        return Some(0.0);
    }
    let mut source_links = vec![false; m];
    let mut target_links = vec![false; n];
    for (i, row) in matrix.iter().enumerate() {
        for (j, score) in row.iter().enumerate() {
            if score.weighted > threshold {
                source_links[i] = true;
                target_links[j] = true;
            }
        }
    }
    Some(
        (source_links.iter().filter(|&&v| v).count() + target_links.iter().filter(|&&v| v).count())
            as f64
            / (m + n) as f64,
    )
}

fn reinforce(matrix: &mut [Vec<LeafScore>], factor: f64, leaf_weight: f64) {
    for score in matrix.iter_mut().flatten() {
        score.structural = (score.structural * factor).min(1.0);
        score.weighted = weighted(score.structural, score.linguistic, leaf_weight);
    }
}

fn datatype_name(dtype: &DataType) -> &'static str {
    match dtype {
        DataType::Unknown => "unknown",
        DataType::Boolean => "boolean",
        DataType::Integer => "int",
        DataType::Float => "float",
        DataType::Decimal => "decimal",
        DataType::Text => "varchar",
        DataType::Date => "date",
        DataType::Timestamp => "timestamp",
    }
}

fn datatype_compatibility(a: &DataType, b: &DataType, cache: &SemanticCache) -> f64 {
    if a == b {
        return 1.0;
    }
    fn family(value: &DataType) -> Option<u8> {
        match value {
            DataType::Integer | DataType::Boolean => Some(1),
            DataType::Float | DataType::Decimal => Some(2),
            DataType::Text => Some(0),
            DataType::Date | DataType::Timestamp => Some(3),
            DataType::Unknown => None,
        }
    }
    match (family(a), family(b)) {
        (Some(a), Some(b)) => f64::from(a == b),
        _ => cache.score(datatype_name(a), datatype_name(b)),
    }
}

// Token types are numbered by their weight group. Symbols have zero weight.
#[derive(Clone, Debug, PartialEq)]
struct Token {
    word: String,
    kind: usize,
}
const TOKEN_WEIGHTS: [f64; 4] = [0.0, 0.1, 0.1, 0.8];

fn normalize(name: &str) -> Vec<Token> {
    let mut result = Vec::new();
    for word in word_tokenize(name) {
        // Python's `token in string.punctuation` also recognizes contiguous
        // punctuation runs that happen to be substrings of that literal.
        if "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".contains(&word) {
            result.push(Token { word, kind: 0 });
        } else if word.parse::<f64>().is_ok() {
            result.push(Token { word, kind: 1 });
        } else {
            let snake = snake_case(&word);
            if snake.contains('_') {
                result.extend(normalize(&snake.replace('_', " ")));
            } else {
                let word = word.to_lowercase();
                let kind = if english_stopword(&word) { 2 } else { 3 };
                result.push(Token { word, kind });
            }
        }
    }
    result
}

fn snake_case(word: &str) -> String {
    static RULES: OnceLock<(Regex, Regex)> = OnceLock::new();
    let (first, second) = RULES.get_or_init(|| {
        (
            Regex::new(r"(.)([A-Z][a-z]+)").expect("constant regex"),
            Regex::new(r"([a-z0-9])([A-Z])").expect("constant regex"),
        )
    });
    let first = first.replace_all(word, "${1}_${2}");
    second.replace_all(&first, "${1}_${2}").to_lowercase()
}

/// Treebank-style word tokenization, including decimal numbers and contractions.
/// Schema labels are treated as one sentence. Natural-language sentence boundary
/// inference (NLTK Punkt) is intentionally not a table-label operation.
fn word_tokenize(name: &str) -> Vec<String> {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (r"([«“‘„]|`+)", " ${1} "),
            (r#"^\""#, "``"),
            (r"(``)", " ${1} "),
            (r#"([ (\[{<])(\"|'')"#, "${1} `` "),
            (r#"([^\.])(\.)([\]\)}>\"'»”’ ]*)\s*$"#, "${1} ${2} ${3} "),
            (r"([:,])([^\d])", " ${1} ${2}"),
            (r"([:,])$", " ${1} "),
            (r"\.{2,}", " ${0} "),
            (r"[;@#$%&]", " ${0} "),
            (r#"([^\.])(\.)([\]\)}>\"']*)\s*$"#, "${1} ${2}${3} "),
            (r"[?!]", " ${0} "),
            (r"([^'])' ", "${1} ' "),
            (r"\*", " ${0} "),
            (r"[\]\[(){}<>]", " ${0} "),
            (r"--", " -- "),
            (r"([»”’])", " ${1} "),
            (r"''", " '' "),
            (r#"\""#, " '' "),
            (r"\s+", " "),
            (r"([^' ])('[sS]|'[mM]|'[dD]|') ", "${1} ${2} "),
            (r"([^' ])('ll|'LL|'re|'RE|'ve|'VE|n't|N'T) ", "${1} ${2} "),
            (r"(?i)\b(can)(not)\b", " ${1} ${2} "),
            (r"(?i)\b(d)('ye)\b", " ${1} ${2} "),
            (r"(?i)\b(gim)(me)\b", " ${1} ${2} "),
            (r"(?i)\b(gon)(na)\b", " ${1} ${2} "),
            (r"(?i)\b(got)(ta)\b", " ${1} ${2} "),
            (r"(?i)\b(lem)(me)\b", " ${1} ${2} "),
            (r"(?i)\b(more)('n)\b", " ${1} ${2} "),
            (r"(?i)\b(wan)(na)(\s)", " ${1} ${2}${3}"),
            (r"(?i) ('t)(is)\b", " ${1} ${2} "),
            (r"(?i) ('t)(was)\b", " ${1} ${2} "),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(pattern).expect("constant tokenizer regex"),
                replacement,
            )
        })
        .collect()
    });
    static CLITIC: OnceLock<Regex> = OnceLock::new();
    let clitic = CLITIC.get_or_init(|| Regex::new(r"(')(\w)\b").expect("constant clitic regex"));
    let mut text = name.to_owned();
    for (i, (pattern, replacement)) in rules.iter().enumerate() {
        if i == 4 {
            // NLTK's starting-quote clitic rule uses negative lookahead,
            // which Rust's regex engine omits. Apply that condition explicitly.
            text = clitic
                .replace_all(&text, |captures: &regex::Captures<'_>| {
                    let letter = captures[2].to_lowercase();
                    if matches!(letter.as_str(), "m" | "t" | "s" | "d" | "n") {
                        captures[0].to_owned()
                    } else {
                        format!("{} {}", &captures[1], &captures[2])
                    }
                })
                .into_owned();
        }
        if i == 15 {
            text = format!(" {text} ");
        }
        text = pattern.replace_all(&text, *replacement).into_owned();
    }
    text.split_whitespace().map(str::to_owned).collect()
}

fn name_similarity(a: &[Token], b: &[Token], cache: &SemanticCache) -> f64 {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (kind, &weight) in TOKEN_WEIGHTS.iter().enumerate().skip(1) {
        let a: Vec<_> = a.iter().filter(|token| token.kind == kind).collect();
        let b: Vec<_> = b.iter().filter(|token| token.kind == kind).collect();
        if a.is_empty() || b.is_empty() {
            continue;
        }
        let partial = |queries: &[&Token], candidates: &[&Token]| -> f64 {
            queries
                .iter()
                .map(|a| {
                    candidates
                        .iter()
                        .map(|b| cache.score(&a.word, &b.word))
                        .fold(0.0, f64::max)
                })
                .sum()
        };
        numerator += weight * (partial(&a, &b) + partial(&b, &a));
        denominator += weight * (a.len() + b.len()) as f64;
    }
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

#[derive(Default)]
struct SemanticCache {
    scores: Mutex<HashMap<(String, String), f64>>,
}

impl SemanticCache {
    fn score(&self, a: &str, b: &str) -> f64 {
        if a == b {
            return 1.0;
        }
        let key = if a <= b {
            (a.to_owned(), b.to_owned())
        } else {
            (b.to_owned(), a.to_owned())
        };
        if let Some(score) = self
            .scores
            .lock()
            .expect("semantic cache lock")
            .get(&key)
            .copied()
        {
            return score;
        }
        let score = word_similarity(&key.0, &key.1);
        self.scores
            .lock()
            .expect("semantic cache lock")
            .insert(key, score);
        score
    }
}

/// Cupid token similarity: maximum WordNet Wu-Palmer score across all senses;
/// normalized Levenshtein is used only when WordNet lacks either lemma.
/// Synset pairs are evaluated in ascending canonical WordNet name order. This
/// fixes the orientation of NLTK's asymmetric root-tie cases independently of
/// caller direction, worker scheduling and prior corpus lookups.
pub fn word_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let db = wordnet();
    let (Some(a_senses), Some(b_senses)) = (db.lemmas.get(a), db.lemmas.get(b)) else {
        return similarity(a, b, StringDistanceFunction::Levenshtein);
    };
    let mut best: Option<f64> = None;
    for &a in a_senses.iter() {
        for &b in b_senses.iter() {
            if let Some(score) = db.wu_palmer(a, b) {
                best = Some(best.map_or(score, |current| current.max(score)));
                if score == 1.0 {
                    return 1.0;
                }
            }
        }
    }
    best.unwrap_or_else(|| similarity(a, b, StringDistanceFunction::Levenshtein))
}

struct Synset {
    noun: bool,
    min_depth: u8,
    max_depth: u8,
    ancestors: std::ops::Range<usize>,
}

struct WordNet {
    synsets: Vec<Synset>,
    ancestry: Vec<(u32, u8)>,
    lemmas: HashMap<Box<str>, Box<[u32]>>,
}

fn wordnet() -> &'static WordNet {
    static CORPUS: OnceLock<WordNet> = OnceLock::new();
    CORPUS.get_or_init(|| WordNet::read(include_bytes!("../../assets/wordnet.bin.gz")))
}

impl WordNet {
    fn read(compressed: &[u8]) -> Self {
        let mut data = Vec::new();
        GzDecoder::new(compressed)
            .read_to_end(&mut data)
            .expect("bundled WordNet corpus is valid gzip");
        let mut reader = BinaryReader {
            data: &data,
            cursor: 0,
        };
        assert_eq!(
            reader.bytes(8),
            b"FKWN0001",
            "bundled WordNet corpus version"
        );
        let count = reader.u32() as usize;
        let mut synsets = Vec::with_capacity(count);
        let mut ancestry = Vec::new();
        for _ in 0..count {
            let noun = reader.u8() != 0;
            let min_depth = reader.u8();
            let max_depth = reader.u8();
            let count = reader.u16() as usize;
            let start = ancestry.len();
            for _ in 0..count {
                ancestry.push((reader.u32(), reader.u8()));
            }
            synsets.push(Synset {
                noun,
                min_depth,
                max_depth,
                ancestors: start..ancestry.len(),
            });
        }
        let count = reader.u32() as usize;
        let mut lemmas = HashMap::with_capacity(count);
        for _ in 0..count {
            let length = reader.u16() as usize;
            let word =
                std::str::from_utf8(reader.bytes(length)).expect("bundled WordNet lemma UTF-8");
            let count = reader.u16() as usize;
            let senses: Vec<_> = (0..count).map(|_| reader.u32()).collect();
            lemmas.insert(word.into(), senses.into_boxed_slice());
        }
        assert_eq!(reader.cursor, data.len(), "bundled WordNet corpus complete");
        Self {
            synsets,
            ancestry,
            lemmas,
        }
    }

    fn ancestors(&self, synset: u32) -> &[(u32, u8)] {
        &self.ancestry[self.synsets[synset as usize].ancestors.clone()]
    }

    fn wu_palmer(&self, a: u32, b: u32) -> Option<f64> {
        if a == b {
            return Some(1.0);
        }
        // build_wordnet.py assigns these IDs in ascending canonical name order.
        // This makes NLTK's first-operand LCS tie preference reproducible across
        // runs, instead of depending on Python object IDs.
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let sa = &self.synsets[a as usize];
        let sb = &self.synsets[b as usize];
        let simulate_root = !sa.noun || !sb.noun;
        let aa = self.ancestors(a);
        let ba = self.ancestors(b);
        let mut candidates = Vec::new();
        let mut maximum_min_depth = if simulate_root { Some(0) } else { None };
        intersect(aa, ba, |common, _, _| {
            let depth = self.synsets[common as usize].min_depth;
            if maximum_min_depth.is_none_or(|maximum| depth > maximum) {
                maximum_min_depth = Some(depth);
                candidates.clear();
            }
            if maximum_min_depth == Some(depth) {
                candidates.push(common);
            }
        });
        let depth = maximum_min_depth?;
        let subsumer = if candidates.contains(&a) {
            Some(a)
        } else if simulate_root && depth == 0 {
            None
        } else {
            candidates.first().copied()
        };
        let (lcs_depth, distance_a, distance_b) = if let Some(subsumer) = subsumer {
            let lcs_depth = u32::from(self.synsets[subsumer as usize].max_depth) + 1;
            (
                lcs_depth,
                self.distance(a, subsumer, simulate_root)?,
                self.distance(b, subsumer, simulate_root)?,
            )
        } else if simulate_root {
            (1, root_distance(aa), root_distance(ba))
        } else {
            return None;
        };
        Some(2.0 * lcs_depth as f64 / (distance_a + distance_b + 2 * lcs_depth) as f64)
    }

    fn distance(&self, a: u32, b: u32, simulate_root: bool) -> Option<u32> {
        if a == b {
            return Some(0);
        }
        let aa = self.ancestors(a);
        let ba = self.ancestors(b);
        let mut best = simulate_root.then(|| root_distance(aa) + root_distance(ba));
        intersect(aa, ba, |_, a, b| {
            let distance = u32::from(a) + u32::from(b);
            best = Some(best.map_or(distance, |current| current.min(distance)));
        });
        best
    }
}

fn root_distance(ancestors: &[(u32, u8)]) -> u32 {
    u32::from(ancestors.iter().map(|&(_, d)| d).max().unwrap_or(0)) + 1
}

fn intersect(a: &[(u32, u8)], b: &[(u32, u8)], mut found: impl FnMut(u32, u8, u8)) {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                found(a[i].0, a[i].1, b[j].1);
                i += 1;
                j += 1;
            }
        }
    }
}

struct BinaryReader<'a> {
    data: &'a [u8],
    cursor: usize,
}
impl<'a> BinaryReader<'a> {
    fn bytes(&mut self, count: usize) -> &'a [u8] {
        let value = &self.data[self.cursor..self.cursor + count];
        self.cursor += count;
        value
    }
    fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes(2).try_into().expect("two bytes"))
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes(4).try_into().expect("four bytes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_matching_uses_entire_wordnet_and_edit_fallback() {
        assert_eq!(word_similarity("car", "automobile"), 1.0);
        assert_eq!(word_similarity("physician", "doctor"), 1.0);
        assert_eq!(word_similarity("zxqabc", "zxqabd"), 5.0 / 6.0);
        assert!(word_similarity("dog", "cat") > 0.8);
        assert!(wordnet().lemmas.len() > 140_000);
    }

    #[test]
    fn normalization_keeps_upstream_token_types_and_duplicates() {
        let tokens = normalize("HelloWorld, 123 and hello");
        let words: Vec<_> = tokens.iter().map(|t| (t.word.as_str(), t.kind)).collect();
        assert_eq!(
            words,
            [
                ("hello", 3),
                ("world", 3),
                (",", 0),
                ("123", 1),
                ("and", 2),
                ("hello", 3)
            ]
        );
        assert_eq!(
            word_tokenize("They'll save $3.88."),
            ["They", "'ll", "save", "$", "3.88", "."]
        );
        assert_eq!(snake_case("CamelCaseX"), "camel_case_x");
    }

    #[test]
    fn typed_tokens_use_max_matching_weighted_dice() {
        let cache = SemanticCache::default();
        // Number and content groups have different weights; the unmatched
        // common-word group is skipped when absent on the other side.
        let score = name_similarity(
            &normalize("car 12 the"),
            &normalize("automobile 13"),
            &cache,
        );
        // WordNet represents numbers too: twelve/thirteen score 7/8.
        assert!((score - (0.8 + 0.1 * 0.875) / 0.9).abs() < 1e-12);
    }
}
