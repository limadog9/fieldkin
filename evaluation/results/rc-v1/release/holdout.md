# Release qualification: holdout

Features, thresholds and labels were frozen before this evaluation. These are synthetic fixture results, not production accuracy estimates. Holdout threshold curves are descriptive and must not be used to tune this release. No selected frontier threshold changes the library default.

## Default threshold 0.70

| Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | No-match proposals | Ambiguous proposals |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| combined | independent | 51/136 | 37.50% | 36.43% | 40.00% | 96.88% | 80/90 | 0/10 |
| combined | one_to_one | 51/136 | 37.50% | 36.43% | 40.00% | 96.88% | 80/90 | 0/10 |
| name_only | independent | 45/135 | 33.33% | 32.14% | 35.71% | 90.62% | 85/90 | 0/10 |
| name_only | one_to_one | 45/135 | 33.33% | 32.14% | 35.71% | 90.62% | 85/90 | 0/10 |

## Descriptive matched-precision comparisons

Nonempty proposal sets only. Among grid points meeting the floor, select most correct matches, then higher precision, then higher threshold. The combined-default anchor is the combined model's .70 precision in this same partition and assignment mode. These frontier rows can select a different threshold even for combined. They are not default-threshold measurements or release recommendations.

| Assignment | Anchor | Precision floor | Model | Status | Threshold | Precision | Recall | Unique coverage |
| --- | --- | ---: | --- | --- | ---: | ---: | ---: | ---: |
| independent | combined_default | 37.50% | combined | feasible | 0.500 | 46.54% | 52.86% | 56.43% |
| independent | combined_default | 37.50% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| independent | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| independent | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | combined_default | 37.50% | combined | feasible | 0.500 | 46.54% | 52.86% | 56.43% |
| one_to_one | combined_default | 37.50% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |

## Default results by domain and scenario

Scenario tags overlap; do not sum tag rows. Variants of a family are correlated. The JSON includes all family and variant counts and per-field default predictions, including failed proposals and abstentions. Candidate retrieval includes ineligible ranked pairs and is often easy on small schemas.

| Group | Model | Assignment | Correct/proposed | Precision | Recall | Expected-abstention accuracy |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| domain: crm_contacts | combined | independent | 25/40 | 62.50% | 55.56% | 0.00% |
| domain: financial_records | combined | independent | 0/40 | 0.00% | 0.00% | 22.22% |
| domain: operational_telemetry | combined | independent | 21/21 | 100.00% | 42.00% | 100.00% |
| domain: product_import | combined | independent | 5/35 | 14.29% | 16.67% | 0.00% |
| scenario_tag: ambiguous | combined | independent | 21/21 | 100.00% | 42.00% | 100.00% |
| scenario_tag: base | combined | independent | 11/28 | 39.29% | 39.29% | 20.00% |
| scenario_tag: constant_samples | combined | independent | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: disjoint_samples | combined | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: duplicate_names | combined | independent | 21/21 | 100.00% | 84.00% | 100.00% |
| scenario_tag: incompatible_types | combined | independent | 0/10 | 0.00% | 0.00% | 66.67% |
| scenario_tag: inconsistent_types | combined | independent | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: low_cardinality | combined | independent | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: misleading_names | combined | independent | 25/40 | 62.50% | 55.56% | 0.00% |
| scenario_tag: misleading_qualifiers | combined | independent | 0/70 | 0.00% | 0.00% | 13.33% |
| scenario_tag: no_match | combined | independent | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: null_heavy | combined | independent | 9/26 | 34.62% | 32.14% | 20.00% |
| scenario_tag: opaque_identifiers | combined | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | combined | independent | 36/56 | 64.29% | 40.00% | 50.00% |
| scenario_tag: reordered | combined | independent | 15/32 | 46.88% | 28.85% | 20.00% |
| scenario_tag: separator_noise | combined | independent | 11/28 | 39.29% | 39.29% | 20.00% |
| scenario_tag: unequal_sizes | combined | independent | 41/56 | 73.21% | 34.17% | 66.67% |
| scenario_tag: unicode_names | combined | independent | 5/5 | 100.00% | 16.67% | n/a |
| scenario_tag: unrelated_schemas | combined | independent | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | combined | independent | 9/26 | 34.62% | 32.14% | 20.00% |
| domain: crm_contacts | combined | one_to_one | 25/40 | 62.50% | 55.56% | 0.00% |
| domain: financial_records | combined | one_to_one | 0/40 | 0.00% | 0.00% | 22.22% |
| domain: operational_telemetry | combined | one_to_one | 21/21 | 100.00% | 42.00% | 100.00% |
| domain: product_import | combined | one_to_one | 5/35 | 14.29% | 16.67% | 0.00% |
| scenario_tag: ambiguous | combined | one_to_one | 21/21 | 100.00% | 42.00% | 100.00% |
| scenario_tag: base | combined | one_to_one | 11/28 | 39.29% | 39.29% | 20.00% |
| scenario_tag: constant_samples | combined | one_to_one | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: disjoint_samples | combined | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: duplicate_names | combined | one_to_one | 21/21 | 100.00% | 84.00% | 100.00% |
| scenario_tag: incompatible_types | combined | one_to_one | 0/10 | 0.00% | 0.00% | 66.67% |
| scenario_tag: inconsistent_types | combined | one_to_one | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: low_cardinality | combined | one_to_one | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: misleading_names | combined | one_to_one | 25/40 | 62.50% | 55.56% | 0.00% |
| scenario_tag: misleading_qualifiers | combined | one_to_one | 0/70 | 0.00% | 0.00% | 13.33% |
| scenario_tag: no_match | combined | one_to_one | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: null_heavy | combined | one_to_one | 9/26 | 34.62% | 32.14% | 20.00% |
| scenario_tag: opaque_identifiers | combined | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | combined | one_to_one | 36/56 | 64.29% | 40.00% | 50.00% |
| scenario_tag: reordered | combined | one_to_one | 15/32 | 46.88% | 28.85% | 20.00% |
| scenario_tag: separator_noise | combined | one_to_one | 11/28 | 39.29% | 39.29% | 20.00% |
| scenario_tag: unequal_sizes | combined | one_to_one | 41/56 | 73.21% | 34.17% | 66.67% |
| scenario_tag: unicode_names | combined | one_to_one | 5/5 | 100.00% | 16.67% | n/a |
| scenario_tag: unrelated_schemas | combined | one_to_one | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | combined | one_to_one | 9/26 | 34.62% | 32.14% | 20.00% |
| domain: crm_contacts | name_only | independent | 25/40 | 62.50% | 55.56% | 0.00% |
| domain: financial_records | name_only | independent | 0/45 | 0.00% | 0.00% | 11.11% |
| domain: operational_telemetry | name_only | independent | 15/15 | 100.00% | 30.00% | 100.00% |
| domain: product_import | name_only | independent | 5/35 | 14.29% | 16.67% | 0.00% |
| scenario_tag: ambiguous | name_only | independent | 15/15 | 100.00% | 30.00% | 100.00% |
| scenario_tag: base | name_only | independent | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: constant_samples | name_only | independent | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: disjoint_samples | name_only | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: duplicate_names | name_only | independent | 15/15 | 100.00% | 60.00% | 100.00% |
| scenario_tag: incompatible_types | name_only | independent | 0/15 | 0.00% | 0.00% | 33.33% |
| scenario_tag: inconsistent_types | name_only | independent | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: low_cardinality | name_only | independent | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: misleading_names | name_only | independent | 25/40 | 62.50% | 55.56% | 0.00% |
| scenario_tag: misleading_qualifiers | name_only | independent | 0/75 | 0.00% | 0.00% | 6.67% |
| scenario_tag: no_match | name_only | independent | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: null_heavy | name_only | independent | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: opaque_identifiers | name_only | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | name_only | independent | 30/55 | 54.55% | 33.33% | 33.33% |
| scenario_tag: reordered | name_only | independent | 13/31 | 41.94% | 25.00% | 15.00% |
| scenario_tag: separator_noise | name_only | independent | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: unequal_sizes | name_only | independent | 35/55 | 63.64% | 29.17% | 50.00% |
| scenario_tag: unicode_names | name_only | independent | 5/5 | 100.00% | 16.67% | n/a |
| scenario_tag: unrelated_schemas | name_only | independent | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | name_only | independent | 9/27 | 33.33% | 32.14% | 15.00% |
| domain: crm_contacts | name_only | one_to_one | 25/40 | 62.50% | 55.56% | 0.00% |
| domain: financial_records | name_only | one_to_one | 0/45 | 0.00% | 0.00% | 11.11% |
| domain: operational_telemetry | name_only | one_to_one | 15/15 | 100.00% | 30.00% | 100.00% |
| domain: product_import | name_only | one_to_one | 5/35 | 14.29% | 16.67% | 0.00% |
| scenario_tag: ambiguous | name_only | one_to_one | 15/15 | 100.00% | 30.00% | 100.00% |
| scenario_tag: base | name_only | one_to_one | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: constant_samples | name_only | one_to_one | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: disjoint_samples | name_only | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: duplicate_names | name_only | one_to_one | 15/15 | 100.00% | 60.00% | 100.00% |
| scenario_tag: incompatible_types | name_only | one_to_one | 0/15 | 0.00% | 0.00% | 33.33% |
| scenario_tag: inconsistent_types | name_only | one_to_one | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: low_cardinality | name_only | one_to_one | 15/80 | 18.75% | 60.00% | 0.00% |
| scenario_tag: misleading_names | name_only | one_to_one | 25/40 | 62.50% | 55.56% | 0.00% |
| scenario_tag: misleading_qualifiers | name_only | one_to_one | 0/75 | 0.00% | 0.00% | 6.67% |
| scenario_tag: no_match | name_only | one_to_one | 10/20 | 50.00% | 50.00% | 0.00% |
| scenario_tag: null_heavy | name_only | one_to_one | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: opaque_identifiers | name_only | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | name_only | one_to_one | 30/55 | 54.55% | 33.33% | 33.33% |
| scenario_tag: reordered | name_only | one_to_one | 13/31 | 41.94% | 25.00% | 15.00% |
| scenario_tag: separator_noise | name_only | one_to_one | 9/27 | 33.33% | 32.14% | 15.00% |
| scenario_tag: unequal_sizes | name_only | one_to_one | 35/55 | 63.64% | 29.17% | 50.00% |
| scenario_tag: unicode_names | name_only | one_to_one | 5/5 | 100.00% | 16.67% | n/a |
| scenario_tag: unrelated_schemas | name_only | one_to_one | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | name_only | one_to_one | 9/27 | 33.33% | 32.14% | 15.00% |

Qualification targets remain precision >=95%, unique coverage >=60%, and candidate recall@5 >=90%. See `quality_targets` in JSON for each combined default result. An unmet target is a release limitation; it is not silently waived. Hidden semantic differences, arbitrary identifiers and missing evidence remain known failure cases.
