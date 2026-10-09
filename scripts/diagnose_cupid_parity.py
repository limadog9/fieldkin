"""Record Cupid's upstream object-address-dependent WordNet tie behavior."""

import json
from pathlib import Path
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

ROOT = Path(__file__).resolve().parent.parent


def repeat(index):
    output = ROOT / f"target/parity/cupid_customers_repeat_{index}.json"
    command = [sys.executable, str(ROOT / "scripts/run_valentine_parity.py"),
               "--dataset", "eval/customers.json", "--algorithm", "cupid", "--output", str(output)]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
    if result.returncode:
        return {"run": index, "error": result.stderr + result.stdout}
    dataset = json.loads(output.read_text())["datasets"][0]
    return {"run": index, "input_sha256": dataset["input_sha256"],
            "matches": dataset["algorithms"]["cupid"]["matches"],
            "command": command[1:]}


def main():
    sys.path[:0] = [str(ROOT / ".upstream/python"), str(ROOT / ".upstream/valentine")]
    import nltk
    nltk.data.path.insert(0, str(ROOT / ".upstream/nltk_data"))
    from nltk.corpus import wordnet as wn
    from valentine.algorithms.cupid import linguistic_matching
    import inspect

    original = json.loads((ROOT / "target/parity/python_results.json").read_text())
    original = next(dataset for dataset in original["datasets"] if dataset["path"] == "eval/customers.json")
    account = wn.synset("account.v.02")
    balance = wn.synset("balance.v.02")
    noun_account = wn.synset("account.n.03")
    noun_balance = wn.synset("balance.n.01")
    asymmetric = {
        "first_synset": account.name(), "second_synset": balance.name(),
        "forward_wu_palmer": account.wup_similarity(balance),
        "reverse_wu_palmer": balance.wup_similarity(account),
        "symmetric_noun_max": noun_account.wup_similarity(noun_balance),
        "first_object_id": id(account), "second_object_id": id(balance),
        "upstream_wup_sim_score": linguistic_matching._wup_sim(account, balance),
    }
    with ThreadPoolExecutor(max_workers=3) as pool:
        repeats = list(pool.map(repeat, [1, 2, 3]))
    report = {
        "dataset_path": original["path"], "input_sha256": original["input_sha256"],
        "original_live_matches": original["algorithms"]["cupid"]["matches"],
        "upstream_source_file": ".upstream/valentine/valentine/algorithms/cupid/linguistic_matching.py",
        "upstream_function": "_wup_sim",
        "upstream_function_source": inspect.getsource(linguistic_matching._wup_sim),
        "wordnet_version": wn.get_version(), "asymmetric_synsets": asymmetric,
        "fresh_same_input_runs": repeats,
        "explanation": "Upstream sorts synsets by Python object id before invoking NLTK Wu-Palmer. "
                       "Wu-Palmer chooses its first operand when it belongs to a tied lowest common hypernym set. "
                       "For account.v.02/balance.v.02 one orientation yields 2/3, the other 2/5. "
                       "The reverse case therefore makes the symmetric noun score 8/13 the maximum. "
                       "Object allocation order can change Cupid's linguistic score with identical input. "
                       "Rust uses a fixed canonical synset order instead.",
    }
    output = ROOT / "target/parity/cupid_diagnostics.json"
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(json.dumps(asymmetric, indent=2))
    for run in repeats:
        scores = [(entry["pair"]["source_column"], entry["pair"]["target_column"], entry["score"])
                  for entry in run.get("matches", [])]
        print("Run", run["run"], scores or run.get("error"))
    print("Wrote", output)


if __name__ == "__main__":
    main()
