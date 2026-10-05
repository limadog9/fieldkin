# Readiness regression

Examined evidence only. Release decision: **not qualified**; no fresh independent qualification was performed. All variants and both assignment modes are retained. Labels reach the scorer after matching. T2D and corrective reserved partitions are untouched.

| Corpus | Model | Assignment | Correct/proposed | Wrong | Abstentions | Precision | Unique coverage | Recall@5 | Retained | Lost | Recovered | New wrong |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| original_development | contextual_relationship_repair | independent | 387/387 | 0 | 573 | 100.00% | 53.01% | 100.00% | 387 | 20 | 0 | 0 |
| original_development | contextual_relationship_repair | one_to_one | 382/382 | 0 | 578 | 100.00% | 52.33% | 100.00% | 382 | 20 | 0 | 0 |
| original_development | contextual_population_repair | independent | 427/429 | 2 | 531 | 99.53% | 58.49% | 100.00% | 407 | 0 | 20 | 2 |
| original_development | contextual_population_repair | one_to_one | 422/424 | 2 | 536 | 99.53% | 57.81% | 100.00% | 402 | 0 | 20 | 2 |
| original_development | contextual_ranking_repair | independent | 407/407 | 0 | 553 | 100.00% | 55.75% | 100.00% | 407 | 0 | 0 | 0 |
| original_development | contextual_ranking_repair | one_to_one | 402/402 | 0 | 558 | 100.00% | 55.07% | 100.00% | 402 | 0 | 0 | 0 |
| original_development | contextual_sampled_identifier_renames | independent | 411/411 | 0 | 549 | 100.00% | 56.30% | 100.00% | 387 | 20 | 24 | 0 |
| original_development | contextual_sampled_identifier_renames | one_to_one | 406/406 | 0 | 554 | 100.00% | 55.62% | 100.00% | 382 | 20 | 24 | 0 |
| original_development | contextual_quality | independent | 431/433 | 2 | 527 | 99.54% | 59.04% | 100.00% | 387 | 20 | 44 | 2 |
| original_development | contextual_quality | one_to_one | 426/428 | 2 | 532 | 99.53% | 58.36% | 100.00% | 382 | 20 | 44 | 2 |
| stage3_extension_without_hints | contextual_relationship_repair | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_relationship_repair | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_population_repair | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_population_repair | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_ranking_repair | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_ranking_repair | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_sampled_identifier_renames | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_sampled_identifier_renames | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_quality | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | 13 | 0 | 0 | 0 |
| stage3_extension_without_hints | contextual_quality | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | 12 | 0 | 0 | 0 |
| corrective_development | contextual_relationship_repair | independent | 63/66 | 3 | 150 | 95.45% | 44.68% | 100.00% | 63 | 0 | 0 | 0 |
| corrective_development | contextual_relationship_repair | one_to_one | 60/63 | 3 | 153 | 95.24% | 42.55% | 100.00% | 60 | 0 | 0 | 0 |
| corrective_development | contextual_population_repair | independent | 66/69 | 3 | 147 | 95.65% | 46.81% | 100.00% | 63 | 0 | 3 | 0 |
| corrective_development | contextual_population_repair | one_to_one | 63/66 | 3 | 150 | 95.45% | 44.68% | 100.00% | 60 | 0 | 3 | 0 |
| corrective_development | contextual_ranking_repair | independent | 63/66 | 3 | 150 | 95.45% | 44.68% | 100.00% | 63 | 0 | 0 | 0 |
| corrective_development | contextual_ranking_repair | one_to_one | 60/63 | 3 | 153 | 95.24% | 42.55% | 100.00% | 60 | 0 | 0 | 0 |
| corrective_development | contextual_sampled_identifier_renames | independent | 78/81 | 3 | 135 | 96.30% | 55.32% | 100.00% | 63 | 0 | 15 | 0 |
| corrective_development | contextual_sampled_identifier_renames | one_to_one | 75/78 | 3 | 138 | 96.15% | 53.19% | 100.00% | 60 | 0 | 15 | 0 |
| corrective_development | contextual_quality | independent | 81/84 | 3 | 132 | 96.43% | 57.45% | 100.00% | 63 | 0 | 18 | 0 |
| corrective_development | contextual_quality | one_to_one | 78/81 | 3 | 135 | 96.30% | 55.32% | 100.00% | 60 | 0 | 18 | 0 |
