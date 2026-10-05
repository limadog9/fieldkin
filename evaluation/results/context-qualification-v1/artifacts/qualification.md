# Prospective contextual qualification

One fixed opt-in runtime candidate, frozen before one newly authored synthetic holdout run. No caller hints or confirmations. All 12 families and five related variants are included; the variants are not independent observations. Labels reach only the scorer after schema matching.

| Model | Assignment | Correct/proposed | Precision | Correct recall | Unique coverage | Candidate recall@5 | Wrong unique | No-match proposals | Ambiguous proposals | Targets met |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| default | independent | 27/27 | 100.00% | 12.86% | 12.86% | 100.00% | 0 | 0 | 0 | false |
| default | one_to_one | 24/24 | 100.00% | 11.43% | 11.43% | 100.00% | 0 | 0 | 0 | false |
| name_only_070 | independent | 15/15 | 100.00% | 7.14% | 7.14% | 90.91% | 0 | 0 | 0 | false |
| name_only_070 | one_to_one | 15/15 | 100.00% | 7.14% | 7.14% | 90.91% | 0 | 0 | 0 | false |
| name_only_exact | independent | 15/15 | 100.00% | 7.14% | 7.14% | 90.91% | 0 | 0 | 0 | false |
| name_only_exact | one_to_one | 15/15 | 100.00% | 7.14% | 7.14% | 90.91% | 0 | 0 | 0 | false |
| context_v3 | independent | 111/111 | 100.00% | 52.86% | 52.86% | 100.00% | 0 | 0 | 0 | false |
| context_v3 | one_to_one | 103/103 | 100.00% | 49.05% | 49.05% | 100.00% | 0 | 0 | 0 | false |

Fixed synthetic quality targets met in both assignment modes: **false**. This is one agent-authored synthetic holdout, not a production accuracy guarantee. Defaults remain unchanged. No holdout-driven tuning is permitted after this report.
