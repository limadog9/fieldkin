# Readiness regression

Examined evidence only. Release decision: **not qualified**; no fresh independent qualification was performed. All variants and both assignment modes are retained. Labels reach the scorer after matching. T2D and corrective reserved partitions are untouched.

| Corpus | Model | Assignment | Correct/proposed | Wrong | Abstentions | Precision | Unique coverage | Recall@5 | Retained | Lost | Recovered | New wrong |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| original_development | contextual_unscoped_support | independent | 436/436 | 0 | 524 | 100.00% | 59.73% | 100.00% | 407 | 0 | 29 | 0 |
| original_development | contextual_unscoped_support | one_to_one | 431/431 | 0 | 529 | 100.00% | 59.04% | 100.00% | 402 | 0 | 29 | 0 |
| original_development | contextual_relaxed_unscoped | independent | 456/459 | 3 | 501 | 99.35% | 62.47% | 100.00% | 407 | 0 | 49 | 3 |
| original_development | contextual_relaxed_unscoped | one_to_one | 451/454 | 3 | 506 | 99.34% | 61.78% | 100.00% | 402 | 0 | 49 | 3 |
| original_development | contextual_quality | independent | 431/433 | 2 | 527 | 99.54% | 59.04% | 100.00% | 387 | 20 | 44 | 2 |
| original_development | contextual_quality | one_to_one | 426/428 | 2 | 532 | 99.53% | 58.36% | 100.00% | 382 | 20 | 44 | 2 |
| stage3_extension_without_hints | contextual_unscoped_support | independent | 13/16 | 3 | 16 | 81.25% | 65.00% | 100.00% | 11 | 2 | 2 | 3 |
| stage3_extension_without_hints | contextual_unscoped_support | one_to_one | 13/16 | 3 | 16 | 81.25% | 65.00% | 100.00% | 11 | 1 | 2 | 3 |
| stage3_extension_without_hints | contextual_relaxed_unscoped | independent | 13/16 | 3 | 16 | 81.25% | 65.00% | 100.00% | 11 | 2 | 2 | 3 |
| stage3_extension_without_hints | contextual_relaxed_unscoped | one_to_one | 13/16 | 3 | 16 | 81.25% | 65.00% | 100.00% | 11 | 1 | 2 | 3 |
| stage3_extension_without_hints | contextual_quality | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_quality | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| corrective_development | contextual_unscoped_support | independent | 67/91 | 24 | 125 | 73.63% | 47.52% | 100.00% | 55 | 8 | 12 | 21 |
| corrective_development | contextual_unscoped_support | one_to_one | 67/91 | 24 | 125 | 73.63% | 47.52% | 100.00% | 55 | 5 | 12 | 21 |
| corrective_development | contextual_relaxed_unscoped | independent | 67/91 | 24 | 125 | 73.63% | 47.52% | 100.00% | 55 | 8 | 12 | 21 |
| corrective_development | contextual_relaxed_unscoped | one_to_one | 67/91 | 24 | 125 | 73.63% | 47.52% | 100.00% | 55 | 5 | 12 | 21 |
| corrective_development | contextual_quality | independent | 81/84 | 3 | 132 | 96.43% | 57.45% | 100.00% | 63 | 0 | 18 | 0 |
| corrective_development | contextual_quality | one_to_one | 78/81 | 3 | 135 | 96.30% | 55.32% | 100.00% | 60 | 0 | 18 | 0 |
