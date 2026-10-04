# T2D annotation-only development evidence

These are induced schema lookups using original T2D annotation headers and class-derived property vocabularies. No raw values or types are supplied. All absent labels are unknown; precision and unmatched-field safety cannot be measured. Holdout classes remain reserved.

| Model | Assignment | Known-positive / proposed | Unknown proposals | Known-positive field recall | Positive edge recall@5 | Rejected tables / fields |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| combined | independent | 0 / 0 | 0 | 0.00% | 72.92% | 0 / 0 |
| combined | one_to_one | 0 / 0 | 0 | 0.00% | 72.92% | 0 / 0 |
| name_only | independent | 336 / 443 | 107 | 23.16% | 72.92% | 0 / 0 |
| name_only | one_to_one | 335 / 442 | 107 | 23.09% | 72.92% | 0 / 0 |

Recall denominators include all 1,451 annotated development fields/edges, including input rejections. Top-5 candidate recall includes ineligible candidates. Known-positive/proposed is a count, not a precision estimate: unannotated choices remain unknown. Per-class counts and every rejected input/proposal are retained in JSON/JSONL. The 218 tables from 19 reserved classes were not scored.
