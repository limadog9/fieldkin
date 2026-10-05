# Northix class-equivalence diagnostic

Fixed 84 native table pairs, with 469 source-field occurrences, 28 labeled positive edges, 441 complement no-match occurrences and 70 explicitly UNCLASSED occurrences. Reused tables are correlated; this historical demonstration-data task does not certify production accuracy. No tuning or T2D holdout scoring.

| Model | Samples | Assignment | Correct / proposed | Precision | Positive field recall | Candidate edge recall@5 | No-match false proposals | Explicit UNCLASSED false proposals | Rejected pairs |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| combined | absent | independent | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| combined | absent | one_to_one | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| combined | present | independent | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| combined | present | one_to_one | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| name_only | absent | independent | 14 / 20 | 70.00% | 50.00% | 92.86% | 6 / 441 | 0 / 70 | 0 |
| name_only | absent | one_to_one | 14 / 20 | 70.00% | 50.00% | 92.86% | 6 / 441 | 0 / 70 | 0 |
| name_only | present | independent | 14 / 20 | 70.00% | 50.00% | 92.86% | 6 / 441 | 0 / 70 | 0 |
| name_only | present | one_to_one | 14 / 20 | 70.00% | 50.00% | 92.86% | 6 / 441 | 0 / 70 | 0 |
| valentine_coma_schema_and_samples | present | independent | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| valentine_coma_schema_and_samples | present | one_to_one | 0 / 0 | n/a | 0.00% | 92.86% | 0 / 441 | 0 / 70 | 0 |
| valentine_coma_schema_only | absent | independent | 17 / 31 | 54.84% | 60.71% | 89.29% | 14 / 441 | 0 / 70 | 0 |
| valentine_coma_schema_only | absent | one_to_one | 17 / 27 | 62.96% | 60.71% | 89.29% | 10 / 441 | 0 / 70 | 0 |

Precision uses published class equivalence as a closed task definition. Complement no-match cases include fields whose class has no representative in this particular target table; explicit UNCLASSED is a separately labeled subset. Rejected inputs stay in recall/no-match denominators and are not successful abstentions. Top-five retrieval includes ineligible candidates, but omitted external-score pairs are never counted as retrieved evidence. Candidate scores are heuristic, not calibrated probabilities. Detailed outcomes, rejected inputs and per-source-table groups remain in JSON/JSONL.
