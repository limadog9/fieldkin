"""Regression checks for the versioned canonical-order Cupid source patch.

    python scripts/test_cupid_reference_fix.py --reference-dir PATH_TO_OVERLAY

Runs the actual patched Python matcher in fresh processes with different corpus
lookup orders. It does not modify a checkout, replace matcher outputs, or use
Rust outputs as the expected result.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import inspect
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def worker(reference_dir, order):
    sys.path[:0] = [str(reference_dir), str(ROOT / ".upstream/python")]
    import nltk
    nltk.data.path.insert(0, str(ROOT / ".upstream/nltk_data"))
    from valentine.algorithms import Cupid
    from valentine.algorithms.cupid import linguistic_matching as linguistic
    from nltk.corpus import wordnet as wn

    source_path = Path(inspect.getfile(linguistic)).resolve()
    if not source_path.is_relative_to(reference_dir):
        raise AssertionError(f"Reference imported outside requested overlay: {source_path}")
    wrapper = inspect.getsource(linguistic._wup_sim)
    if "s1.name() > s2.name()" not in wrapper or "id(s1)" in wrapper:
        raise AssertionError("Reference does not contain the canonical-order source patch")

    # Import shared adapters only after loading the overlay's package, so the
    # classes remain bound to the independently imported patched implementation.
    sys.argv.append("--reference-fixes")
    from run_valentine_parity import InputTable
    if order == "account_first":
        linguistic._cached_synsets("account")
        linguistic._cached_synsets("balance")
    elif order == "balance_first":
        linguistic._cached_synsets("balance")
        linguistic._cached_synsets("account")
    else:
        for word in ("car", "doctor", "division", "department", "employee", "worker", "amount", "net"):
            linguistic._cached_synsets(word)
    account = wn.synset("account.v.02")
    balance = wn.synset("balance.v.02")
    forward = linguistic._wup_sim(account, balance)
    reverse = linguistic._wup_sim(balance, account)
    assert forward == reverse == 2 / 3
    assert account.wup_similarity(balance) == 2 / 3
    assert balance.wup_similarity(account) == 2 / 5
    assert linguistic._token_similarity("account", "balance") == 2 / 3
    assert linguistic._token_similarity("balance", "account") == 2 / 3

    raw = (ROOT / "eval/customers.json").read_bytes()
    data = json.loads(raw)
    source = InputTable("source", data["source"]["fields"])
    target = InputTable("target", data["target"]["fields"])
    scores = []
    for config, expected in [({}, 0.7767211111111111),
                              ({"leaf_w_struct": 0.4, "th_accept": 0.5}, 0.6645533333333333)]:
        matches = Cupid(**config).get_matches(source, target)
        pair, score = next((pair, score) for pair, score in matches.items()
                           if pair.source_column == "balance" and pair.target_column == "account_balance")
        assert abs(score - expected) < 1e-12, (config, expected, score)
        scores.append({"config": config, "score": float(score), "expected": expected})
    # Exercise a warm token cache independently of how the synsets were loaded.
    linguistic._token_similarity("department", "division")
    assert linguistic._token_similarity("account", "balance") == 2 / 3
    return {"lookup_order": order, "input_sha256": hashlib.sha256(raw).hexdigest(),
            "reference_source_file": str(source_path),
            "reference_source_sha256": hashlib.sha256(source_path.read_bytes()).hexdigest(),
            "wordnet_version": wn.get_version(), "scores": scores,
            "canonical_forward_wu_palmer": forward, "canonical_reverse_wu_palmer": reverse}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/parity/resolved/cupid_regression.json")
    parser.add_argument("--worker", choices=("account_first", "balance_first", "other_words_first"))
    args = parser.parse_args()
    reference_dir = args.reference_dir.resolve()
    if args.worker:
        print(json.dumps(worker(reference_dir, args.worker), sort_keys=True))
        return

    def run(order):
        command = [sys.executable, str(Path(__file__).resolve()), "--reference-dir", str(reference_dir),
                   "--worker", order]
        process = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
        if process.returncode:
            raise RuntimeError(process.stdout + process.stderr)
        return json.loads(process.stdout)

    orders = ["account_first", "balance_first", "other_words_first"]
    with ThreadPoolExecutor(max_workers=3) as pool:
        runs = list(pool.map(run, orders))
    report = {
        "patch": "scripts/reference_patches/cupid.patch",
        "patch_sha256": hashlib.sha256((ROOT / "scripts/reference_patches/cupid.patch").read_bytes()).hexdigest(),
        "specification": "Order each WordNet synset pair by ascending canonical synset.name() before NLTK Wu-Palmer",
        "runs": runs,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Passed 3 fresh process lookup orders and 2 customer configurations; wrote {args.output}")


if __name__ == "__main__":
    main()
