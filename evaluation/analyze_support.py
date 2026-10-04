#!/usr/bin/env python3
"""Inspect selected proposals in the published RC-v1 reports, without scoring.

Python 3.9+, standard library only. Inputs are fixed to the original published
development and holdout reports. The old holdout is now regression evidence;
this script never reads newly authored fixtures or invokes the matching engine.
"""

import argparse
from collections import Counter
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parent.parent
PUBLISHED = ROOT / "evaluation" / "results" / "rc-v1" / "release"
FREEZE = "5d5c3746032ac743a9130d46b2794ed4e4efa074"


def summarize(records, unique_fields):
    """Count old selected proposals retained by a diagnostic predicate."""
    outcomes = Counter(record["outcome"] for record in records)
    correct = outcomes["correct_unique_proposal"]
    return {
        "proposals": len(records),
        "correct_proposals": correct,
        "incorrect_proposals": len(records) - correct,
        "precision": correct / len(records) if records else None,
        "unique_recall": correct / unique_fields if unique_fields else None,
        "wrong_unique_proposals": outcomes["wrong_unique_proposal"],
        "false_no_match_proposals": outcomes["false_no_match_proposal"],
        "unsafe_ambiguous_proposals": outcomes["unsafe_ambiguous_proposal"],
    }


def has_support(record, name_floor=None, sample_floor=0.5):
    samples = record["scores"]["samples"]
    if samples is None or samples < sample_floor:
        return False
    name = record["scores"]["name"]
    return name_floor is None or (name is not None and name >= name_floor)


def analyze(partition):
    path = PUBLISHED / f"{partition}.json"
    report = json.loads(path.read_text(encoding="utf8"))
    metadata = report["metadata"]
    if metadata["protocol"]["feature_freeze_revision"] != FREEZE:
        raise ValueError("input does not identify the original published RC-v1 freeze")
    if metadata["partition"] != partition:
        raise ValueError("input partition does not match the published report filename")
    # Independent selections suffice for this fixed descriptive calculation.
    # Global reassignment is deliberately not simulated by filtering selections.
    predictions = [
        prediction
        for prediction in report["default_predictions"]
        if prediction["model"] == "combined"
        and prediction["assignment"] == "independent"
    ]
    unique_fields = sum(prediction["gold_kind"] == "match" for prediction in predictions)
    selected = []
    for prediction in predictions:
        choice = prediction["selected"]
        if choice is None:
            continue
        candidate = next(
            (candidate for candidate in prediction["candidates"] if candidate["target"] == choice["target"]),
            None,
        )
        if candidate is None:
            raise ValueError("selected candidate support is absent; cannot classify every proposal")
        scores = {signal["name"]: signal["score"] for signal in candidate["signals"]}
        if set(scores) != {"name", "type", "samples"}:
            raise ValueError("published combined signal inventory changed")
        selected.append({"outcome": prediction["outcome"], "scores": scores})

    default = next(
        row
        for row in report["threshold_rows"]
        if row["model"] == "combined"
        and row["assignment"] == "independent"
        and row["threshold"] == 0.7
    )
    original = summarize(selected, unique_fields)
    if (
        len(predictions) != default["counts"]["fields"]
        or unique_fields != default["counts"]["unique_fields"]
        or original["proposals"] != default["counts"]["proposals"]
        or original["correct_proposals"] != default["counts"]["correct_proposals"]
    ):
        raise ValueError("selected prediction totals disagree with the published aggregate")

    perfect = [record for record in selected if all(value == 1.0 for value in record["scores"].values())]
    return {
        "partition": partition,
        "input": str(path.relative_to(ROOT)).replace("\\", "/"),
        "model": "combined",
        "assignment": "independent",
        "published_threshold": 0.7,
        "fields": len(predictions),
        "unique_fields": unique_fields,
        "published_default": original,
        "all_three_signal_scores_equal_one": summarize(perfect, unique_fields),
        "selected_proposal_postfilters": {
            "sample_at_least_0.50": summarize(
                [record for record in selected if has_support(record)], unique_fields
            ),
            "name_at_least_0.80_and_sample_at_least_0.50": summarize(
                [record for record in selected if has_support(record, name_floor=0.8)], unique_fields
            ),
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, help="Optional new JSON file; defaults to stdout")
    args = parser.parse_args()
    result = {
        "analysis": "published-rc-v1-selected-support-v1",
        "source_freeze": FREEZE,
        "engine_calls": 0,
        "new_holdout_read": False,
        "interpretation": (
            "Descriptive postfilters of already selected original RC-v1 independent proposals. "
            "They do not recompute candidate eligibility, local ambiguity or assignment, and "
            "are not an evaluation of a new engine policy. Filtering candidates can remove a "
            "tie and create a proposal absent from these old selections. Perfect existing "
            "signal scores do not resolve the recorded semantic errors or prove equivalence. "
            "The original holdout is now published regression evidence."
        ),
        "results": [analyze(partition) for partition in ("development", "holdout")],
    }
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        with args.output.open("x", encoding="utf8", newline="\n") as output:
            output.write(rendered)
    else:
        sys.stdout.write(rendered)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, StopIteration) as error:
        sys.exit(f"Support analysis failed: {error}")
