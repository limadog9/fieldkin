# Readiness regression

Examined evidence only. Release decision: **not qualified**; no fresh independent qualification was performed. All variants and both assignment modes are retained. Labels reach the scorer after matching. T2D and corrective reserved partitions are untouched.

| Corpus | Model | Assignment | Correct/proposed | Wrong | Abstentions | Precision | Unique coverage | Recall@5 | Retained | Lost | Recovered | New wrong |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| original_development | default | independent | 321/422 | 101 | 538 | 76.07% | 44.66% | 100.00% | null | null | null | null |
| original_development | default | one_to_one | 313/414 | 101 | 546 | 75.60% | 43.56% | 100.00% | null | null | null | null |
| original_development | name_only_070 | independent | 275/375 | 100 | 585 | 73.33% | 38.36% | 97.53% | null | null | null | null |
| original_development | name_only_070 | one_to_one | 270/370 | 100 | 590 | 72.97% | 37.67% | 97.53% | null | null | null | null |
| original_development | name_only_exact | independent | 270/360 | 90 | 600 | 75.00% | 37.67% | 97.53% | null | null | null | null |
| original_development | name_only_exact | one_to_one | 265/355 | 90 | 605 | 74.65% | 36.99% | 97.53% | null | null | null | null |
| original_development | contextual | independent | 407/407 | 0 | 553 | 100.00% | 55.75% | 100.00% | null | null | null | null |
| original_development | contextual | one_to_one | 402/402 | 0 | 558 | 100.00% | 55.07% | 100.00% | null | null | null | null |
| original_development | contextual_quality | independent | 407/409 | 2 | 551 | 99.51% | 55.75% | 100.00% | null | null | null | null |
| original_development | contextual_quality | one_to_one | 402/404 | 2 | 556 | 99.50% | 55.07% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | default | independent | 16/26 | 10 | 6 | 61.54% | 80.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | default | one_to_one | 15/25 | 10 | 7 | 60.00% | 75.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | name_only_070 | independent | 14/24 | 10 | 8 | 58.33% | 70.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | name_only_070 | one_to_one | 13/23 | 10 | 9 | 56.52% | 65.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | name_only_exact | independent | 14/24 | 10 | 8 | 58.33% | 70.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | name_only_exact | one_to_one | 13/23 | 10 | 9 | 56.52% | 65.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | contextual | independent | 13/13 | 0 | 19 | 100.00% | 65.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | contextual | one_to_one | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | contextual_quality | independent | 12/12 | 0 | 20 | 100.00% | 60.00% | 100.00% | null | null | null | null |
| stage3_extension_without_hints | contextual_quality | one_to_one | 11/11 | 0 | 21 | 100.00% | 55.00% | 100.00% | null | null | null | null |
| corrective_development | default | independent | 20/74 | 54 | 142 | 27.03% | 14.18% | 100.00% | null | null | null | null |
| corrective_development | default | one_to_one | 20/74 | 54 | 142 | 27.03% | 14.18% | 100.00% | null | null | null | null |
| corrective_development | name_only_070 | independent | 0/54 | 54 | 162 | 0.00% | 0.00% | 94.55% | null | null | null | null |
| corrective_development | name_only_070 | one_to_one | 0/54 | 54 | 162 | 0.00% | 0.00% | 94.55% | null | null | null | null |
| corrective_development | name_only_exact | independent | 0/54 | 54 | 162 | 0.00% | 0.00% | 94.55% | null | null | null | null |
| corrective_development | name_only_exact | one_to_one | 0/54 | 54 | 162 | 0.00% | 0.00% | 94.55% | null | null | null | null |
| corrective_development | contextual | independent | 63/66 | 3 | 150 | 95.45% | 44.68% | 100.00% | null | null | null | null |
| corrective_development | contextual | one_to_one | 60/63 | 3 | 153 | 95.24% | 42.55% | 100.00% | null | null | null | null |
| corrective_development | contextual_quality | independent | 66/69 | 3 | 147 | 95.65% | 46.81% | 100.00% | null | null | null | null |
| corrective_development | contextual_quality | one_to_one | 63/66 | 3 | 150 | 95.45% | 44.68% | 100.00% | null | null | null | null |
