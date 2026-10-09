"""Generate checked-in native-port oracle data from the upstream Python code.

Clone delftdata/valentine into .upstream/valentine and install its dependencies
into .upstream/python (or the active Python environment), then run this script
from the repository root. Python is only used to regenerate these test fixtures;
the Rust library and its test suite do not invoke Python.
"""

from __future__ import annotations

import json
from pathlib import Path
import random
import subprocess
import sys
from types import ModuleType

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT / ".upstream/python"), str(ROOT / ".upstream/valentine")]

# Skip eager package imports of unrelated algorithms and NLTK corpora. Every
# scorer, helper and reduction below still comes directly from upstream files.
for package in ["valentine", "valentine.algorithms", "valentine.data_sources",
                "valentine.algorithms.coma", "valentine.algorithms.coma.similarity"]:
    module = ModuleType(package)
    module.__path__ = [str(ROOT / ".upstream/valentine" / package.replace(".", "/"))]
    sys.modules[package] = module

import rapidfuzz
from rapidfuzz.distance import DamerauLevenshtein, Hamming, Jaro, JaroWinkler, Levenshtein
from valentine.algorithms.jaccard_distance import StringDistanceFunction
from valentine.algorithms.jaccard_distance.jaccard_distance import JaccardDistanceMatcher
from valentine.algorithms.coma.similarity.tokens import tokenize_name, tokens_similarity
from valentine.algorithms.coma.similarity.trigram import trigram_similarity

DISTANCES = {
    "levenshtein": (StringDistanceFunction.Levenshtein, Levenshtein.normalized_similarity),
    "damerau_levenshtein": (
        StringDistanceFunction.DamerauLevenshtein, DamerauLevenshtein.normalized_similarity
    ),
    "hamming": (StringDistanceFunction.Hamming, Hamming.normalized_similarity),
    "jaro": (StringDistanceFunction.Jaro, Jaro.normalized_similarity),
    "jaro_winkler": (StringDistanceFunction.JaroWinkler, JaroWinkler.normalized_similarity),
    "exact": (StringDistanceFunction.Exact, lambda a, b: float(a == b)),
}


def set_score(source: list[str], target: list[str], config: dict) -> float:
    matcher = JaccardDistanceMatcher(
        distance_fun=DISTANCES[config["distance_fun"]][0],
        threshold_dist=config["threshold_dist"],
        tversky_alpha=config["tversky_alpha"],
        tversky_beta=config["tversky_beta"],
    )
    matches = matcher.process_jaccard_distance((
        source, target, config["threshold_dist"], "target", "value", "source", "value",
        DISTANCES[config["distance_fun"]][0], None,
    ))
    return float(next(iter(matches.values())))


def generate() -> dict:
    rng = random.Random(649087)
    string_pairs = [
        ("", ""), ("", "x"), ("x", ""), ("a", "b"), ("ab", "abc"),
        ("CA", "ABC"), ("kitten", "sitting"), ("MARTHA", "MARHTA"),
        ("DWAYNE", "DUANE"), ("café", "cafe"), ("東京🙂", "東京🙃"),
        ("🙂界a", "界🙂a"), ("aébc", "éabc"), ("abcdefghij", "abcdefgXYZ"),
        ("\u0130", "i\u0307"), ("aaaa", "aaab"),
    ]
    alphabet = "abcdé界🙂"
    for _ in range(80):
        string_pairs.append(tuple(
            "".join(rng.choice(alphabet) for _ in range(rng.randrange(0, 12)))
            for _ in range(2)
        ))
    distances = [
        {"function": name, "left": left, "right": right, "score": scorer(left, right)}
        for name, (_, scorer) in DISTANCES.items()
        for left, right in string_pairs
    ]
    value_pairs = [
        ([], []), ([], ["a"]), ([""], [""]), (["x"], ["y"]),
        (["a", "b", "a"], ["b", "c"]), (["cat"], ["cat", "cats", "catsx"]),
        (["cat", "cats", "catsx"], ["cat"]), (["x"], ["x", "y", "z"]),
        (["東京", "東京🙂"], ["東京", "東京🙃"]), ([" Paris"], ["Paris"]),
        (["abcdefgXYZ"], ["abcdefghij"]),
        (["abcde"], ["abcdz"]),
        (["abc", "abd", "xyz"], ["abe", "uvw", "xyz", "abc"]),
    ]
    for _ in range(20):
        value_pairs.append(tuple(
            [rng.choice(string_pairs)[rng.randrange(2)] for _ in range(rng.randrange(0, 7))]
            for _ in range(2)
        ))
    sets = []
    for name in DISTANCES:
        for threshold in [0.0, 0.6, 0.7, 0.75, 0.8, 1.0]:
            for alpha, beta in [(1.0, 1.0), (1.0, 0.0), (0.25, 0.75), (0.0, 0.0)]:
                config = {
                    "distance_fun": name, "threshold_dist": threshold,
                    "tversky_alpha": alpha, "tversky_beta": beta,
                }
                for source, target in value_pairs:
                    sets.append({
                        "config": config, "source": source, "target": target,
                        "score": set_score(source, target, config),
                    })
    names = [
        "XMLHTTPRequest42_id_ID", "Date١٢_value", "ApproxDate", "date_created_approximation",
        "mgr", "manager", "fname", "firstname", "at", "attention", "cat", "customer",
        "", "é界", "É界", "aaaa", "aaa", "HTTPSURL", "iso8601Date", "²Ⅷfield",
    ]
    commit = subprocess.run(
        ["git", "-C", str(ROOT / ".upstream/valentine"), "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    return {
        "upstream_commit": commit,
        "rapidfuzz_version": rapidfuzz.__version__,
        "distances": distances,
        "sets": sets,
        "tokens": [{"name": name, "tokens": list(tokenize_name(name))} for name in names],
        "names": [
            {"left": left, "right": right, "tokens": tokens_similarity(left, right),
             "trigrams": trigram_similarity(left, right)}
            for left in names for right in names
        ],
    }


if __name__ == "__main__":
    destination = ROOT / "tests/fixtures/jaccard.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    fixture = generate()
    destination.write_text(json.dumps(fixture, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"Wrote {len(fixture['distances'])} distances, {len(fixture['sets'])} set scores, "
          f"{len(fixture['names'])} name pairs to {destination.relative_to(ROOT)}")
