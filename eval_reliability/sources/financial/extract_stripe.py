#!/usr/bin/env python3
"""Regenerate blind financial schema case; standard library only, no matcher imports.

Default: verify archived publisher excerpt and regenerate case + evidence.
--download: retrieve immutable publisher OpenAPI, verify its checksum, rebuild
the unchanged excerpt, and then generate. No publisher records are synthesized.
"""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parent
REVISION = "7d04b7f1e0a6f49768e3b46edbfd5de810b59373"
URL = f"https://raw.githubusercontent.com/stripe/openapi/{REVISION}/openapi/spec3.json"
FULL_SHA256 = "7cff4cc46d0654101a36a3302f7544640f5f73e4f3cad6f264f0e23c19fa1776"
EXCERPT_SHA256 = "3c962236341fe67e6242e3c6ff51c3e568714b703be9ccc26daa92cc5d10dc7c"
NAMES = ["plan", "price", "recurring", "plan_tier", "price_tier", "transform_usage", "transform_quantity", "custom_unit_amount"]

# Publisher schema names remain exact. Dots indicate nesting, [] an array item.
# The source/target entity names reside in the manifest, not synthetic columns.
MATCHES = [
    ("active", "active"),
    ("amount", "unit_amount"),
    ("billing_scheme", "billing_scheme"),
    ("currency", "currency"),
    ("interval", "recurring.interval"),
    ("interval_count", "recurring.interval_count"),
    ("livemode", "livemode"),
    ("meter", "recurring.meter"),
    ("nickname", "nickname"),
    ("product", "product"),
    ("tiers[].unit_amount", "tiers[].unit_amount"),
    ("tiers[].up_to", "tiers[].up_to"),
    ("tiers_mode", "tiers_mode"),
    ("transform_usage.divide_by", "transform_quantity.divide_by"),
    ("transform_usage.round", "transform_quantity.round"),
    ("usage_type", "recurring.usage_type"),
]
NO_MATCH = {
    "created": "Target projection omits price.created. Object creation time is not a billing interval, trial duration, quantity threshold, or amount.",
    "id": "Target projection omits price.id. The plan's identifier is not the referenced product identifier, meter identifier, or static lookup key.",
    "tiers[].flat_amount": "Target projection omits price.tiers[].flat_amount and its decimal representation. A price for the entire tier is not the price per unit in that tier or the customer's permitted unit amount.",
    "tiers[].flat_amount_decimal": "Target projection omits both flat-amount representations. This decimal price for an entire tier is not the per-unit price of a tier or the base per-unit amount.",
    "trial_period_days": "No trial-period property appears in the published Price recurring schema. Default free-trial days are distinct from billing interval_count, which counts intervals, and divide_by, which transforms usage quantities.",
}
TARGET_EXTRAS = ["custom_unit_amount.maximum", "custom_unit_amount.minimum", "custom_unit_amount.preset", "lookup_key", "tax_behavior", "type"]


def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    excerpt_path = ROOT / "raw/stripe_schema_excerpt.json"
    if args.download:
        data = urllib.request.urlopen(URL, timeout=90).read()
        assert digest(data) == FULL_SHA256, "Publisher resource checksum mismatch"
        source = json.loads(data)
        dump(excerpt_path, {"info": source["info"], "components": {"schemas": {n: source["components"]["schemas"][n] for n in NAMES}}})
    data = excerpt_path.read_bytes()
    assert digest(data) == EXCERPT_SHA256, "Archived excerpt checksum mismatch"
    spec = json.loads(data)
    schemas = spec["components"]["schemas"]

    def resolve(node, pointer):
        while True:
            if "$ref" in node:
                pointer = node["$ref"]
                node = schemas[pointer.rsplit("/", 1)[-1]]
            elif "anyOf" in node and len(node["anyOf"]) == 1:
                node = node["anyOf"][0]
                pointer += "/anyOf/0"
            else:
                return node, pointer

    def field(entity, path):
        node, pointer = schemas[entity], f"#/components/schemas/{entity}"
        trace = [pointer]
        for part in path.split("."):
            node, pointer = resolve(node, pointer)
            key = part[:-2] if part.endswith("[]") else part
            node, pointer = node["properties"][key], pointer + "/properties/" + key
            trace.append(pointer)
            if part.endswith("[]"):
                node, pointer = resolve(node["items"], pointer + "/items")
                trace.append(pointer)
        declared = node.get("type")
        if declared is None and path == "product":
            assert node["anyOf"][0]["type"] == "string"
            declared = "string"
            variant = "Unexpanded publisher-declared product ID string variant (anyOf/0); expanded product objects are outside this projection."
        else:
            variant = None
        dtype = {"integer": "integer", "string": "text", "boolean": "boolean", "number": "float"}[declared]
        output = {"name": path, "data_type": dtype, "samples": []}
        evidence = {"name": path, "publisher_entity": entity, "schema_pointer": pointer, "reference_trace": trace, "declared_schema": node, "normalized_data_type": dtype, "samples_origin": "none: schema-only missing-evidence control"}
        if variant:
            evidence["projection_constraint"] = variant
        return output, evidence

    source_names = sorted([a for a, _ in MATCHES] + list(NO_MATCH))
    target_names = sorted([b for _, b in MATCHES] + TARGET_EXTRAS)
    source_data = [field("plan", name) for name in source_names]
    target_data = [field("price", name) for name in target_names]
    answers = [{"kind": "match", "source": a, "target": b} for a, b in MATCHES]
    answers += [{"kind": "no_match", "source": a} for a in NO_MATCH]
    answers.sort(key=lambda a: a["source"])
    case = {"name": "financial_stripe_plan_price_schema_projection", "source": {"fields": [x[0] for x in source_data]}, "target": {"fields": [x[0] for x in target_data]}, "answers": answers}
    evidence = {
        "case": case["name"],
        "publisher": "Stripe, Inc.",
        "domain": "financial/subscription billing",
        "evidence_class": "publisher-declared schema only; no observed or synthetic records",
        "selection_policy": "SELECTION_POLICY.md, recorded before retrieval and predictions",
        "repository": "https://github.com/stripe/openapi",
        "revision": REVISION,
        "api_version": spec["info"]["version"],
        "retrieval_date_utc": "2026-10-12",
        "license": "MIT; complete publisher license archived at raw/stripe_LICENSE",
        "resources": [
            {"url": URL, "sha256": FULL_SHA256, "archived_excerpt": "raw/stripe_schema_excerpt.json", "excerpt_sha256": EXCERPT_SHA256, "excerpt_rule": "Keep original info and exactly these complete component schemas in this order: " + ", ".join(NAMES)},
            {"url": f"https://raw.githubusercontent.com/stripe/openapi/{REVISION}/LICENSE", "path": "raw/stripe_LICENSE", "sha256": digest((ROOT / "raw/stripe_LICENSE").read_bytes())},
            {"url": f"https://raw.githubusercontent.com/stripe/openapi/{REVISION}/README.md", "path": "raw/stripe_README.md", "sha256": digest((ROOT / "raw/stripe_README.md").read_bytes())},
        ],
        "equivalence_basis": {"pointer": "#/components/schemas/plan/description", "quote": schemas["plan"]["description"], "interpretation": "Publisher states Prices replaces Plans and is backwards compatible. Retained positive aliases have matching property definitions, units, and roles in that documented migration. For base and tier per-unit amounts, only the integer representation remains on both sides; alternate decimal representations are excluded rather than assuming unique storage-preserving labels."},
        "projection": {
            "naming": "Exact publisher property names, with dots representing nested property traversal and [] representing an array element. No aliases, invented leaf names, or synthetic observations introduced.",
            "source_root": "#/components/schemas/plan",
            "target_root": "#/components/schemas/price",
            "constraint": "Recurring Price objects corresponding to legacy Plans; product fields use the declared unexpanded string ID variant.",
            "target_deliberately_omitted": ["created", "id", "tiers[].flat_amount", "tiers[].flat_amount_decimal"],
            "alternate_representation_exclusions": {"source": ["amount_decimal", "tiers[].unit_amount_decimal"], "target": ["unit_amount_decimal", "tiers[].unit_amount_decimal"], "reason": "Integer and decimal-string properties represent the same economic amount, but decimal strings support fractional precision not necessarily available in integer fields. Without records, unconditional unique or ambiguous cross-representation labels are not established; only one representation per corresponding quantity is retained."},
            "other_omitted_fields": "Source and target object discriminators, free-form metadata maps, and currency-option maps are excluded. Nonretained schema properties are outside this explicit projection.",
            "source_fields": source_names,
            "target_fields": target_names,
        },
        "field_evidence": {"source": [x[1] for x in source_data], "target": [x[1] for x in target_data]},
        "label_evidence": [
            {"source": a, "target": b, "kind": "match", "rationale": "Same role, unit, and definition in the documented backwards-compatible Plan-to-Price migration.", "source_definition": field("plan", a)[1]["declared_schema"]["description"], "target_definition": field("price", b)[1]["declared_schema"]["description"]}
            for a, b in MATCHES
        ] + [{"source": a, "kind": "no_match", "rationale": why} for a, why in NO_MATCH.items()],
        "limitations": ["No observed record distributions; all samples are deliberately empty.", "The benchmark projection is curated and does not claim a whole-schema migration.", "Alternate decimal representations of retained base and tier per-unit quantities are excluded from both projections because schema-only evidence cannot establish unconditional cross-representation equivalence.", "Dotted and [] paths expose publisher structure; they are not native CSV column names."],
        "regenerate": "python3 extract_stripe.py; use --download to verify and re-extract the immutable publisher resource",
    }
    assert len(source_names) == len(set(source_names)) == len(answers)
    assert len(target_names) == len(set(target_names))
    assert {a["source"] for a in answers} == set(source_names)
    assert all(a["target"] in target_names for a in answers if a["kind"] == "match")
    dump(ROOT / "stripe_plan_price.json", case)
    dump(ROOT / "stripe_manifest.json", evidence)
    print(f"Generated {len(source_names)} source fields, {len(target_names)} target fields, {len(MATCHES)} matches, {len(NO_MATCH)} no-match labels; no predictions.")


if __name__ == "__main__":
    main()
