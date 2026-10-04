# Release qualification: development

Features, thresholds and labels were frozen before this evaluation. These are synthetic fixture results, not production accuracy estimates. Holdout threshold curves are descriptive and must not be used to tune this release. No selected frontier threshold changes the library default.

## Default threshold 0.70

| Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | No-match proposals | Ambiguous proposals |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| combined | independent | 321/422 | 76.07% | 43.97% | 44.66% | 100.00% | 96/190 | 0/40 |
| combined | one_to_one | 313/414 | 75.60% | 42.88% | 43.56% | 100.00% | 96/190 | 0/40 |
| name_only | independent | 275/390 | 70.51% | 37.67% | 38.36% | 97.53% | 110/190 | 0/40 |
| name_only | one_to_one | 270/385 | 70.13% | 36.99% | 37.67% | 97.53% | 110/190 | 0/40 |

## Descriptive matched-precision comparisons

Nonempty proposal sets only. Among grid points meeting the floor, select most correct matches, then higher precision, then higher threshold. The combined-default anchor is the combined model's .70 precision in this same partition and assignment mode. These frontier rows can select a different threshold even for combined. They are not default-threshold measurements or release recommendations.

| Assignment | Anchor | Precision floor | Model | Status | Threshold | Precision | Recall | Unique coverage |
| --- | --- | ---: | --- | --- | ---: | ---: | ---: | ---: |
| independent | combined_default | 76.07% | combined | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| independent | combined_default | 76.07% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| independent | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| independent | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | combined_default | 75.60% | combined | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| one_to_one | combined_default | 75.60% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| one_to_one | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |

## Default results by domain and scenario

Scenario tags overlap; do not sum tag rows. Variants of a family are correlated. The JSON includes all family and variant counts and per-field default predictions, including failed proposals and abstentions. Candidate retrieval includes ineligible ranked pairs and is often easy on small schemas.

| Group | Model | Assignment | Correct/proposed | Precision | Recall | Expected-abstention accuracy |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| domain: crm_contacts | combined | independent | 124/167 | 74.25% | 72.94% | 45.71% |
| domain: financial_records | combined | independent | 41/49 | 83.67% | 21.03% | 82.22% |
| domain: operational_telemetry | combined | independent | 125/170 | 73.53% | 73.53% | 35.71% |
| domain: product_import | combined | independent | 31/36 | 86.11% | 15.90% | 88.89% |
| scenario_tag: ambiguous | combined | independent | 51/51 | 100.00% | 34.00% | 100.00% |
| scenario_tag: assignment_collision | combined | independent | 76/76 | 100.00% | 52.41% | 100.00% |
| scenario_tag: base | combined | independent | 73/94 | 77.66% | 50.00% | 56.52% |
| scenario_tag: constant_samples | combined | independent | 32/70 | 45.71% | 27.83% | 41.54% |
| scenario_tag: duplicate_names | combined | independent | 65/65 | 100.00% | 59.09% | 100.00% |
| scenario_tag: duplicate_source_semantics | combined | independent | 8/8 | 100.00% | 32.00% | 100.00% |
| scenario_tag: empty_samples | combined | independent | 60/60 | 100.00% | 44.44% | 100.00% |
| scenario_tag: identifier_scope | combined | independent | 30/53 | 56.60% | 85.71% | 28.00% |
| scenario_tag: incompatible_types | combined | independent | 14/14 | 100.00% | 17.50% | 100.00% |
| scenario_tag: inconsistent_types | combined | independent | 14/14 | 100.00% | 28.00% | 100.00% |
| scenario_tag: insufficient_samples | combined | independent | 13/13 | 100.00% | 13.00% | 100.00% |
| scenario_tag: low_cardinality | combined | independent | 75/138 | 54.35% | 50.00% | 35.56% |
| scenario_tag: misleading_names | combined | independent | 58/146 | 39.73% | 77.33% | 20.95% |
| scenario_tag: misleading_qualifiers | combined | independent | 39/52 | 75.00% | 19.50% | 81.43% |
| scenario_tag: missing_samples | combined | independent | 15/15 | 100.00% | 33.33% | 100.00% |
| scenario_tag: no_match | combined | independent | 73/161 | 45.34% | 63.48% | 33.60% |
| scenario_tag: null_heavy | combined | independent | 79/98 | 80.61% | 32.64% | 74.29% |
| scenario_tag: opaque_identifiers | combined | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | combined | independent | 224/237 | 94.51% | 43.92% | 78.33% |
| scenario_tag: reordered | combined | independent | 125/146 | 85.62% | 51.65% | 56.52% |
| scenario_tag: separator_noise | combined | independent | 73/94 | 77.66% | 50.00% | 56.52% |
| scenario_tag: unequal_sizes | combined | independent | 188/198 | 94.95% | 36.86% | 88.89% |
| scenario_tag: unicode | combined | independent | 50/50 | 100.00% | 83.33% | n/a |
| scenario_tag: units | combined | independent | 53/58 | 91.38% | 75.71% | 75.00% |
| scenario_tag: unknown_types | combined | independent | 30/30 | 100.00% | 66.67% | 100.00% |
| scenario_tag: unrelated_schemas | combined | independent | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | combined | independent | 51/70 | 72.86% | 34.93% | 60.87% |
| domain: crm_contacts | combined | one_to_one | 119/162 | 73.46% | 70.00% | 45.71% |
| domain: financial_records | combined | one_to_one | 41/49 | 83.67% | 21.03% | 82.22% |
| domain: operational_telemetry | combined | one_to_one | 122/167 | 73.05% | 71.76% | 35.71% |
| domain: product_import | combined | one_to_one | 31/36 | 86.11% | 15.90% | 88.89% |
| scenario_tag: ambiguous | combined | one_to_one | 51/51 | 100.00% | 34.00% | 100.00% |
| scenario_tag: assignment_collision | combined | one_to_one | 68/68 | 100.00% | 46.90% | 100.00% |
| scenario_tag: base | combined | one_to_one | 71/92 | 77.17% | 48.63% | 56.52% |
| scenario_tag: constant_samples | combined | one_to_one | 32/70 | 45.71% | 27.83% | 41.54% |
| scenario_tag: duplicate_names | combined | one_to_one | 60/60 | 100.00% | 54.55% | 100.00% |
| scenario_tag: duplicate_source_semantics | combined | one_to_one | 8/8 | 100.00% | 32.00% | 100.00% |
| scenario_tag: empty_samples | combined | one_to_one | 60/60 | 100.00% | 44.44% | 100.00% |
| scenario_tag: identifier_scope | combined | one_to_one | 30/53 | 56.60% | 85.71% | 28.00% |
| scenario_tag: incompatible_types | combined | one_to_one | 14/14 | 100.00% | 17.50% | 100.00% |
| scenario_tag: inconsistent_types | combined | one_to_one | 14/14 | 100.00% | 28.00% | 100.00% |
| scenario_tag: insufficient_samples | combined | one_to_one | 13/13 | 100.00% | 13.00% | 100.00% |
| scenario_tag: low_cardinality | combined | one_to_one | 75/138 | 54.35% | 50.00% | 35.56% |
| scenario_tag: misleading_names | combined | one_to_one | 58/146 | 39.73% | 77.33% | 20.95% |
| scenario_tag: misleading_qualifiers | combined | one_to_one | 39/52 | 75.00% | 19.50% | 81.43% |
| scenario_tag: missing_samples | combined | one_to_one | 15/15 | 100.00% | 33.33% | 100.00% |
| scenario_tag: no_match | combined | one_to_one | 73/161 | 45.34% | 63.48% | 33.60% |
| scenario_tag: null_heavy | combined | one_to_one | 78/97 | 80.41% | 32.23% | 74.29% |
| scenario_tag: opaque_identifiers | combined | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | combined | one_to_one | 216/229 | 94.32% | 42.35% | 78.33% |
| scenario_tag: reordered | combined | one_to_one | 123/144 | 85.42% | 50.83% | 56.52% |
| scenario_tag: separator_noise | combined | one_to_one | 71/92 | 77.17% | 48.63% | 56.52% |
| scenario_tag: unequal_sizes | combined | one_to_one | 180/190 | 94.74% | 35.29% | 88.89% |
| scenario_tag: unicode | combined | one_to_one | 50/50 | 100.00% | 83.33% | n/a |
| scenario_tag: units | combined | one_to_one | 53/58 | 91.38% | 75.71% | 75.00% |
| scenario_tag: unknown_types | combined | one_to_one | 30/30 | 100.00% | 66.67% | 100.00% |
| scenario_tag: unrelated_schemas | combined | one_to_one | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | combined | one_to_one | 50/69 | 72.46% | 34.25% | 60.87% |
| domain: crm_contacts | name_only | independent | 125/170 | 73.53% | 73.53% | 42.86% |
| domain: financial_records | name_only | independent | 20/30 | 66.67% | 10.26% | 77.78% |
| domain: operational_telemetry | name_only | independent | 115/165 | 69.70% | 67.65% | 28.57% |
| domain: product_import | name_only | independent | 15/25 | 60.00% | 7.69% | 77.78% |
| scenario_tag: ambiguous | name_only | independent | 50/55 | 90.91% | 33.33% | 91.67% |
| scenario_tag: assignment_collision | name_only | independent | 55/55 | 100.00% | 37.93% | 100.00% |
| scenario_tag: base | name_only | independent | 55/78 | 70.51% | 37.67% | 52.17% |
| scenario_tag: constant_samples | name_only | independent | 30/65 | 46.15% | 26.09% | 46.15% |
| scenario_tag: duplicate_names | name_only | independent | 50/50 | 100.00% | 45.45% | 100.00% |
| scenario_tag: duplicate_source_semantics | name_only | independent | 5/5 | 100.00% | 20.00% | 100.00% |
| scenario_tag: empty_samples | name_only | independent | 75/75 | 100.00% | 55.56% | 100.00% |
| scenario_tag: identifier_scope | name_only | independent | 30/55 | 54.55% | 85.71% | 20.00% |
| scenario_tag: incompatible_types | name_only | independent | 5/20 | 25.00% | 6.25% | 62.50% |
| scenario_tag: inconsistent_types | name_only | independent | 5/5 | 100.00% | 10.00% | 100.00% |
| scenario_tag: insufficient_samples | name_only | independent | 10/10 | 100.00% | 10.00% | 100.00% |
| scenario_tag: low_cardinality | name_only | independent | 65/130 | 50.00% | 43.33% | 33.33% |
| scenario_tag: misleading_names | name_only | independent | 55/150 | 36.67% | 73.33% | 14.29% |
| scenario_tag: misleading_qualifiers | name_only | independent | 20/40 | 50.00% | 10.00% | 71.43% |
| scenario_tag: missing_samples | name_only | independent | 25/25 | 100.00% | 55.56% | 100.00% |
| scenario_tag: no_match | name_only | independent | 75/170 | 44.12% | 65.22% | 28.00% |
| scenario_tag: null_heavy | name_only | independent | 95/118 | 80.51% | 39.26% | 68.57% |
| scenario_tag: opaque_identifiers | name_only | independent | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | name_only | independent | 170/185 | 91.89% | 33.33% | 75.00% |
| scenario_tag: reordered | name_only | independent | 95/118 | 80.51% | 39.26% | 52.17% |
| scenario_tag: separator_noise | name_only | independent | 55/78 | 70.51% | 37.67% | 52.17% |
| scenario_tag: unequal_sizes | name_only | independent | 160/175 | 91.43% | 31.37% | 83.33% |
| scenario_tag: unicode | name_only | independent | 50/50 | 100.00% | 83.33% | n/a |
| scenario_tag: units | name_only | independent | 50/60 | 83.33% | 71.43% | 50.00% |
| scenario_tag: unknown_types | name_only | independent | 45/45 | 100.00% | 100.00% | 100.00% |
| scenario_tag: unrelated_schemas | name_only | independent | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | name_only | independent | 55/78 | 70.51% | 37.67% | 52.17% |
| domain: crm_contacts | name_only | one_to_one | 120/165 | 72.73% | 70.59% | 42.86% |
| domain: financial_records | name_only | one_to_one | 20/30 | 66.67% | 10.26% | 77.78% |
| domain: operational_telemetry | name_only | one_to_one | 115/165 | 69.70% | 67.65% | 28.57% |
| domain: product_import | name_only | one_to_one | 15/25 | 60.00% | 7.69% | 77.78% |
| scenario_tag: ambiguous | name_only | one_to_one | 50/55 | 90.91% | 33.33% | 91.67% |
| scenario_tag: assignment_collision | name_only | one_to_one | 50/50 | 100.00% | 34.48% | 100.00% |
| scenario_tag: base | name_only | one_to_one | 54/77 | 70.13% | 36.99% | 52.17% |
| scenario_tag: constant_samples | name_only | one_to_one | 30/65 | 46.15% | 26.09% | 46.15% |
| scenario_tag: duplicate_names | name_only | one_to_one | 45/45 | 100.00% | 40.91% | 100.00% |
| scenario_tag: duplicate_source_semantics | name_only | one_to_one | 5/5 | 100.00% | 20.00% | 100.00% |
| scenario_tag: empty_samples | name_only | one_to_one | 75/75 | 100.00% | 55.56% | 100.00% |
| scenario_tag: identifier_scope | name_only | one_to_one | 30/55 | 54.55% | 85.71% | 20.00% |
| scenario_tag: incompatible_types | name_only | one_to_one | 5/20 | 25.00% | 6.25% | 62.50% |
| scenario_tag: inconsistent_types | name_only | one_to_one | 5/5 | 100.00% | 10.00% | 100.00% |
| scenario_tag: insufficient_samples | name_only | one_to_one | 10/10 | 100.00% | 10.00% | 100.00% |
| scenario_tag: low_cardinality | name_only | one_to_one | 65/130 | 50.00% | 43.33% | 33.33% |
| scenario_tag: misleading_names | name_only | one_to_one | 55/150 | 36.67% | 73.33% | 14.29% |
| scenario_tag: misleading_qualifiers | name_only | one_to_one | 20/40 | 50.00% | 10.00% | 71.43% |
| scenario_tag: missing_samples | name_only | one_to_one | 25/25 | 100.00% | 55.56% | 100.00% |
| scenario_tag: no_match | name_only | one_to_one | 75/170 | 44.12% | 65.22% | 28.00% |
| scenario_tag: null_heavy | name_only | one_to_one | 94/117 | 80.34% | 38.84% | 68.57% |
| scenario_tag: opaque_identifiers | name_only | one_to_one | 0/0 | n/a | 0.00% | 100.00% |
| scenario_tag: renamed | name_only | one_to_one | 165/180 | 91.67% | 32.35% | 75.00% |
| scenario_tag: reordered | name_only | one_to_one | 94/117 | 80.34% | 38.84% | 52.17% |
| scenario_tag: separator_noise | name_only | one_to_one | 54/77 | 70.13% | 36.99% | 52.17% |
| scenario_tag: unequal_sizes | name_only | one_to_one | 155/170 | 91.18% | 30.39% | 83.33% |
| scenario_tag: unicode | name_only | one_to_one | 50/50 | 100.00% | 83.33% | n/a |
| scenario_tag: units | name_only | one_to_one | 50/60 | 83.33% | 71.43% | 50.00% |
| scenario_tag: unknown_types | name_only | one_to_one | 45/45 | 100.00% | 100.00% | 100.00% |
| scenario_tag: unrelated_schemas | name_only | one_to_one | 0/60 | 0.00% | n/a | 0.00% |
| scenario_tag: without_samples | name_only | one_to_one | 54/77 | 70.13% | 36.99% | 52.17% |

Qualification targets remain precision >=95%, unique coverage >=60%, and candidate recall@5 >=90%. See `quality_targets` in JSON for each combined default result. An unmet target is a release limitation; it is not silently waived. Hidden semantic differences, arbitrary identifiers and missing evidence remain known failure cases.
