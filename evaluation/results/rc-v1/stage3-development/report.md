# Stage 3 development evidence evaluation

No held-out schema family was scored. These are descriptive development results on public synthetic data, not production accuracy estimates. Thresholds and labels were fixed before scoring. Scores remain uncalibrated heuristics.

## Default threshold

| Corpus | Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | No-match proposals |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| original_development | combined | independent | 321/422 | 76.07% | 43.97% | 44.66% | 100.00% | 96/190 |
| original_development | combined | one_to_one | 313/414 | 75.60% | 42.88% | 43.56% | 100.00% | 96/190 |
| original_development | no_cardinality | independent | 333/438 | 76.03% | 45.62% | 46.30% | 100.00% | 100/190 |
| original_development | no_cardinality | one_to_one | 325/430 | 75.58% | 44.52% | 45.21% | 100.00% | 100/190 |
| original_development | name_only | independent | 275/390 | 70.51% | 37.67% | 38.36% | 97.53% | 110/190 |
| original_development | name_only | one_to_one | 270/385 | 70.13% | 36.99% | 37.67% | 97.53% | 110/190 |
| original_development | profile | independent | 303/396 | 76.52% | 41.51% | 42.19% | 100.00% | 88/190 |
| original_development | profile | one_to_one | 298/391 | 76.21% | 40.82% | 41.51% | 100.00% | 88/190 |
| original_development | no_profile | independent | 291/384 | 75.78% | 39.86% | 40.55% | 100.00% | 88/190 |
| original_development | no_profile | one_to_one | 286/379 | 75.46% | 39.18% | 39.86% | 100.00% | 88/190 |
| original_development | no_aliases | independent | 316/417 | 75.78% | 43.29% | 43.97% | 100.00% | 96/190 |
| original_development | no_aliases | one_to_one | 308/409 | 75.31% | 42.19% | 42.88% | 100.00% | 96/190 |
| original_development | no_hints | independent | 321/422 | 76.07% | 43.97% | 44.66% | 100.00% | 96/190 |
| original_development | no_hints | one_to_one | 313/414 | 75.60% | 42.88% | 43.56% | 100.00% | 96/190 |
| stage3_extension | combined | independent | 16/23 | 69.57% | 80.00% | 80.00% | 100.00% | 7/11 |
| stage3_extension | combined | one_to_one | 15/22 | 68.18% | 75.00% | 75.00% | 100.00% | 7/11 |
| stage3_extension | no_cardinality | independent | 16/23 | 69.57% | 80.00% | 80.00% | 100.00% | 7/11 |
| stage3_extension | no_cardinality | one_to_one | 15/22 | 68.18% | 75.00% | 75.00% | 100.00% | 7/11 |
| stage3_extension | name_only | independent | 14/24 | 58.33% | 70.00% | 70.00% | 100.00% | 10/11 |
| stage3_extension | name_only | one_to_one | 13/23 | 56.52% | 65.00% | 65.00% | 100.00% | 10/11 |
| stage3_extension | profile | independent | 16/23 | 69.57% | 80.00% | 80.00% | 100.00% | 7/11 |
| stage3_extension | profile | one_to_one | 15/22 | 68.18% | 75.00% | 75.00% | 100.00% | 7/11 |
| stage3_extension | no_profile | independent | 16/23 | 69.57% | 80.00% | 80.00% | 100.00% | 7/11 |
| stage3_extension | no_profile | one_to_one | 15/22 | 68.18% | 75.00% | 75.00% | 100.00% | 7/11 |
| stage3_extension | no_aliases | independent | 14/21 | 66.67% | 70.00% | 70.00% | 100.00% | 7/11 |
| stage3_extension | no_aliases | one_to_one | 13/20 | 65.00% | 65.00% | 65.00% | 100.00% | 7/11 |
| stage3_extension | no_hints | independent | 16/26 | 61.54% | 80.00% | 80.00% | 100.00% | 10/11 |
| stage3_extension | no_hints | one_to_one | 15/25 | 60.00% | 75.00% | 75.00% | 100.00% | 10/11 |

## Prespecified matched-precision operating points

Choose maximum correct recall among qualifying grid points; ties use greater precision, then higher threshold. A baseline anchor uses that baseline's .70 default precision. A candidate-default anchor compares baselines at least as precise as that candidate. Undefined precision or no feasible threshold is never counted as a success. Threshold selections below are development analyses and do not alter library defaults.

| Corpus | Assignment | Precision anchor | Floor | Model | Status | Threshold | Precision | Recall | Unique coverage |
| --- | --- | --- | ---: | --- | --- | ---: | ---: | ---: | ---: |
| original_development | independent | fixed_0.80 | 80.00% | combined | feasible | 0.575 | 80.04% | 60.41% | 61.10% |
| original_development | independent | fixed_0.80 | 80.00% | no_cardinality | feasible | 0.550 | 80.03% | 63.70% | 64.38% |
| original_development | independent | fixed_0.80 | 80.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.80 | 80.00% | profile | feasible | 0.525 | 80.57% | 62.47% | 63.15% |
| original_development | independent | fixed_0.80 | 80.00% | no_profile | feasible | 0.500 | 80.24% | 64.52% | 65.21% |
| original_development | independent | fixed_0.80 | 80.00% | no_aliases | feasible | 0.575 | 80.00% | 60.27% | 60.96% |
| original_development | independent | fixed_0.80 | 80.00% | no_hints | feasible | 0.575 | 80.04% | 60.41% | 61.10% |
| original_development | independent | fixed_0.90 | 90.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | no_profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | no_aliases | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.90 | 90.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | no_profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | no_aliases | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | fixed_0.95 | 95.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | no_cardinality_default | 76.03% | combined | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | no_cardinality_default | 76.03% | no_cardinality | feasible | 0.500 | 77.57% | 69.18% | 71.92% |
| original_development | independent | no_cardinality_default | 76.03% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | no_cardinality_default | 76.03% | profile | feasible | 0.500 | 79.83% | 64.52% | 65.21% |
| original_development | independent | no_cardinality_default | 76.03% | no_profile | feasible | 0.500 | 80.24% | 64.52% | 65.21% |
| original_development | independent | no_cardinality_default | 76.03% | no_aliases | feasible | 0.500 | 77.93% | 68.22% | 70.96% |
| original_development | independent | no_cardinality_default | 76.03% | no_hints | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | name_only_default | 70.51% | combined | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | name_only_default | 70.51% | no_cardinality | feasible | 0.500 | 77.57% | 69.18% | 71.92% |
| original_development | independent | name_only_default | 70.51% | name_only | feasible | 0.500 | 71.05% | 55.48% | 58.22% |
| original_development | independent | name_only_default | 70.51% | profile | feasible | 0.500 | 79.83% | 64.52% | 65.21% |
| original_development | independent | name_only_default | 70.51% | no_profile | feasible | 0.500 | 80.24% | 64.52% | 65.21% |
| original_development | independent | name_only_default | 70.51% | no_aliases | feasible | 0.500 | 77.93% | 68.22% | 70.96% |
| original_development | independent | name_only_default | 70.51% | no_hints | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | combined_default | 76.07% | combined | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | combined_default | 76.07% | no_cardinality | feasible | 0.500 | 77.57% | 69.18% | 71.92% |
| original_development | independent | combined_default | 76.07% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | combined_default | 76.07% | profile | feasible | 0.500 | 79.83% | 64.52% | 65.21% |
| original_development | independent | combined_default | 76.07% | no_profile | feasible | 0.500 | 80.24% | 64.52% | 65.21% |
| original_development | independent | combined_default | 76.07% | no_aliases | feasible | 0.500 | 77.93% | 68.22% | 70.96% |
| original_development | independent | combined_default | 76.07% | no_hints | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | profile_default | 76.52% | combined | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | independent | profile_default | 76.52% | no_cardinality | feasible | 0.500 | 77.57% | 69.18% | 71.92% |
| original_development | independent | profile_default | 76.52% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | independent | profile_default | 76.52% | profile | feasible | 0.500 | 79.83% | 64.52% | 65.21% |
| original_development | independent | profile_default | 76.52% | no_profile | feasible | 0.500 | 80.24% | 64.52% | 65.21% |
| original_development | independent | profile_default | 76.52% | no_aliases | feasible | 0.500 | 77.93% | 68.22% | 70.96% |
| original_development | independent | profile_default | 76.52% | no_hints | feasible | 0.500 | 77.33% | 68.22% | 70.96% |
| original_development | one_to_one | fixed_0.80 | 80.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.80 | 80.00% | no_cardinality | feasible | 0.575 | 80.00% | 60.27% | 60.96% |
| original_development | one_to_one | fixed_0.80 | 80.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.80 | 80.00% | profile | feasible | 0.525 | 80.25% | 61.23% | 61.92% |
| original_development | one_to_one | fixed_0.80 | 80.00% | no_profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.80 | 80.00% | no_aliases | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.80 | 80.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | no_profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | no_aliases | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.90 | 90.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | combined | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | no_profile | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | no_aliases | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | fixed_0.95 | 95.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | no_cardinality_default | 75.58% | combined | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | no_cardinality_default | 75.58% | no_cardinality | feasible | 0.500 | 76.77% | 66.99% | 70.14% |
| original_development | one_to_one | no_cardinality_default | 75.58% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | no_cardinality_default | 75.58% | profile | feasible | 0.500 | 79.51% | 62.74% | 63.42% |
| original_development | one_to_one | no_cardinality_default | 75.58% | no_profile | feasible | 0.500 | 79.93% | 62.74% | 63.42% |
| original_development | one_to_one | no_cardinality_default | 75.58% | no_aliases | feasible | 0.500 | 77.12% | 66.03% | 69.18% |
| original_development | one_to_one | no_cardinality_default | 75.58% | no_hints | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | name_only_default | 70.13% | combined | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | name_only_default | 70.13% | no_cardinality | feasible | 0.500 | 76.77% | 66.99% | 70.14% |
| original_development | one_to_one | name_only_default | 70.13% | name_only | feasible | 0.500 | 71.82% | 54.11% | 56.85% |
| original_development | one_to_one | name_only_default | 70.13% | profile | feasible | 0.500 | 79.51% | 62.74% | 63.42% |
| original_development | one_to_one | name_only_default | 70.13% | no_profile | feasible | 0.500 | 79.93% | 62.74% | 63.42% |
| original_development | one_to_one | name_only_default | 70.13% | no_aliases | feasible | 0.500 | 77.12% | 66.03% | 69.18% |
| original_development | one_to_one | name_only_default | 70.13% | no_hints | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | combined_default | 75.60% | combined | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | combined_default | 75.60% | no_cardinality | feasible | 0.500 | 76.77% | 66.99% | 70.14% |
| original_development | one_to_one | combined_default | 75.60% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | combined_default | 75.60% | profile | feasible | 0.500 | 79.51% | 62.74% | 63.42% |
| original_development | one_to_one | combined_default | 75.60% | no_profile | feasible | 0.500 | 79.93% | 62.74% | 63.42% |
| original_development | one_to_one | combined_default | 75.60% | no_aliases | feasible | 0.500 | 77.12% | 66.03% | 69.18% |
| original_development | one_to_one | combined_default | 75.60% | no_hints | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | profile_default | 76.21% | combined | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| original_development | one_to_one | profile_default | 76.21% | no_cardinality | feasible | 0.500 | 76.77% | 66.99% | 70.14% |
| original_development | one_to_one | profile_default | 76.21% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| original_development | one_to_one | profile_default | 76.21% | profile | feasible | 0.500 | 79.51% | 62.74% | 63.42% |
| original_development | one_to_one | profile_default | 76.21% | no_profile | feasible | 0.500 | 79.93% | 62.74% | 63.42% |
| original_development | one_to_one | profile_default | 76.21% | no_aliases | feasible | 0.500 | 77.12% | 66.03% | 69.18% |
| original_development | one_to_one | profile_default | 76.21% | no_hints | feasible | 0.500 | 76.51% | 66.03% | 69.18% |
| stage3_extension | independent | fixed_0.80 | 80.00% | combined | feasible | 0.925 | 92.86% | 65.00% | 65.00% |
| stage3_extension | independent | fixed_0.80 | 80.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.80 | 80.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.80 | 80.00% | profile | feasible | 0.825 | 81.25% | 65.00% | 65.00% |
| stage3_extension | independent | fixed_0.80 | 80.00% | no_profile | feasible | 0.825 | 92.86% | 65.00% | 65.00% |
| stage3_extension | independent | fixed_0.80 | 80.00% | no_aliases | feasible | 0.925 | 91.67% | 55.00% | 55.00% |
| stage3_extension | independent | fixed_0.80 | 80.00% | no_hints | feasible | 1.000 | 80.00% | 60.00% | 60.00% |
| stage3_extension | independent | fixed_0.90 | 90.00% | combined | feasible | 0.925 | 92.86% | 65.00% | 65.00% |
| stage3_extension | independent | fixed_0.90 | 90.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.90 | 90.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.90 | 90.00% | profile | feasible | 0.900 | 100.00% | 60.00% | 60.00% |
| stage3_extension | independent | fixed_0.90 | 90.00% | no_profile | feasible | 0.825 | 92.86% | 65.00% | 65.00% |
| stage3_extension | independent | fixed_0.90 | 90.00% | no_aliases | feasible | 0.925 | 91.67% | 55.00% | 55.00% |
| stage3_extension | independent | fixed_0.90 | 90.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.95 | 95.00% | combined | feasible | 1.000 | 100.00% | 60.00% | 60.00% |
| stage3_extension | independent | fixed_0.95 | 95.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | fixed_0.95 | 95.00% | profile | feasible | 0.900 | 100.00% | 60.00% | 60.00% |
| stage3_extension | independent | fixed_0.95 | 95.00% | no_profile | feasible | 0.900 | 100.00% | 60.00% | 60.00% |
| stage3_extension | independent | fixed_0.95 | 95.00% | no_aliases | feasible | 1.000 | 100.00% | 50.00% | 50.00% |
| stage3_extension | independent | fixed_0.95 | 95.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | no_cardinality_default | 69.57% | combined | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | no_cardinality_default | 69.57% | no_cardinality | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | no_cardinality_default | 69.57% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | no_cardinality_default | 69.57% | profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | no_cardinality_default | 69.57% | no_profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | no_cardinality_default | 69.57% | no_aliases | feasible | 0.500 | 70.83% | 85.00% | 85.00% |
| stage3_extension | independent | no_cardinality_default | 69.57% | no_hints | feasible | 0.925 | 76.47% | 65.00% | 65.00% |
| stage3_extension | independent | name_only_default | 58.33% | combined | feasible | 0.550 | 68.00% | 85.00% | 85.00% |
| stage3_extension | independent | name_only_default | 58.33% | no_cardinality | feasible | 0.575 | 68.00% | 85.00% | 85.00% |
| stage3_extension | independent | name_only_default | 58.33% | name_only | feasible | 0.525 | 60.00% | 75.00% | 75.00% |
| stage3_extension | independent | name_only_default | 58.33% | profile | feasible | 0.500 | 68.00% | 85.00% | 85.00% |
| stage3_extension | independent | name_only_default | 58.33% | no_profile | feasible | 0.500 | 68.00% | 85.00% | 85.00% |
| stage3_extension | independent | name_only_default | 58.33% | no_aliases | feasible | 0.500 | 70.83% | 85.00% | 85.00% |
| stage3_extension | independent | name_only_default | 58.33% | no_hints | feasible | 0.550 | 60.71% | 85.00% | 85.00% |
| stage3_extension | independent | combined_default | 69.57% | combined | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | combined_default | 69.57% | no_cardinality | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | combined_default | 69.57% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | combined_default | 69.57% | profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | combined_default | 69.57% | no_profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | combined_default | 69.57% | no_aliases | feasible | 0.500 | 70.83% | 85.00% | 85.00% |
| stage3_extension | independent | combined_default | 69.57% | no_hints | feasible | 0.925 | 76.47% | 65.00% | 65.00% |
| stage3_extension | independent | profile_default | 69.57% | combined | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | profile_default | 69.57% | no_cardinality | feasible | 0.850 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | profile_default | 69.57% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | independent | profile_default | 69.57% | profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | profile_default | 69.57% | no_profile | feasible | 0.750 | 69.57% | 80.00% | 80.00% |
| stage3_extension | independent | profile_default | 69.57% | no_aliases | feasible | 0.500 | 70.83% | 85.00% | 85.00% |
| stage3_extension | independent | profile_default | 69.57% | no_hints | feasible | 0.925 | 76.47% | 65.00% | 65.00% |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | combined | feasible | 0.925 | 92.31% | 60.00% | 60.00% |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | profile | feasible | 0.825 | 80.00% | 60.00% | 60.00% |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | no_profile | feasible | 0.825 | 92.31% | 60.00% | 60.00% |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | no_aliases | feasible | 0.925 | 90.91% | 50.00% | 50.00% |
| stage3_extension | one_to_one | fixed_0.80 | 80.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | combined | feasible | 0.925 | 92.31% | 60.00% | 60.00% |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | profile | feasible | 0.900 | 100.00% | 55.00% | 55.00% |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | no_profile | feasible | 0.825 | 92.31% | 60.00% | 60.00% |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | no_aliases | feasible | 0.925 | 90.91% | 50.00% | 50.00% |
| stage3_extension | one_to_one | fixed_0.90 | 90.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | combined | feasible | 1.000 | 100.00% | 55.00% | 55.00% |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | no_cardinality | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | profile | feasible | 0.900 | 100.00% | 55.00% | 55.00% |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | no_profile | feasible | 0.900 | 100.00% | 55.00% | 55.00% |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | no_aliases | feasible | 1.000 | 100.00% | 45.00% | 45.00% |
| stage3_extension | one_to_one | fixed_0.95 | 95.00% | no_hints | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | combined | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | no_cardinality | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | no_profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | no_aliases | feasible | 0.500 | 69.57% | 80.00% | 80.00% |
| stage3_extension | one_to_one | no_cardinality_default | 68.18% | no_hints | feasible | 0.925 | 75.00% | 60.00% | 60.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | combined | feasible | 0.550 | 66.67% | 80.00% | 80.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | no_cardinality | feasible | 0.575 | 66.67% | 80.00% | 80.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | name_only | feasible | 0.525 | 58.33% | 70.00% | 70.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | profile | feasible | 0.500 | 66.67% | 80.00% | 80.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | no_profile | feasible | 0.500 | 66.67% | 80.00% | 80.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | no_aliases | feasible | 0.500 | 69.57% | 80.00% | 80.00% |
| stage3_extension | one_to_one | name_only_default | 56.52% | no_hints | feasible | 0.550 | 59.26% | 80.00% | 80.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | combined | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | no_cardinality | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | combined_default | 68.18% | profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | no_profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | no_aliases | feasible | 0.500 | 69.57% | 80.00% | 80.00% |
| stage3_extension | one_to_one | combined_default | 68.18% | no_hints | feasible | 0.925 | 75.00% | 60.00% | 60.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | combined | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | no_cardinality | feasible | 0.850 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | name_only | no_feasible_threshold | n/a | n/a | n/a | n/a |
| stage3_extension | one_to_one | profile_default | 68.18% | profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | no_profile | feasible | 0.750 | 68.18% | 75.00% | 75.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | no_aliases | feasible | 0.500 | 69.57% | 80.00% | 80.00% |
| stage3_extension | one_to_one | profile_default | 68.18% | no_hints | feasible | 0.925 | 75.00% | 60.00% | 60.00% |

Full threshold curves and per-family default counts are in report.json. Original development and new extension results are deliberately not pooled. The extension is small and designed to exercise implementation assumptions; labels are not independent statistical samples, and candidate recall is easy when schemas contain few targets. A semantic-hint ablation measures extra caller knowledge, not better inference from the original inputs. The profile is optional and shape agreement cannot establish meaning.
