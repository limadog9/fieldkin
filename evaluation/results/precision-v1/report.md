# Fixed precision policies on original development

160 unchanged cases / 960 labeled decisions: 730 unique, 190 unmatched, 40 ambiguous. No holdout scored. Default counts and all 320 complete report digests verified against the frozen current-default baseline. No score, threshold, label, split or alias tuning.

| Policy | Assignment | Correct/proposed | Precision | Unique coverage | Overall coverage | Wrong unique | Unmatched proposals | Ambiguous proposals | Lost correct | Removed false |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| default | independent | 321/422 | 76.07% | 44.66% | 43.96% | 5 | 96/190 | 0/40 | 0 | 0 |
| conflicts | independent | 321/417 | 76.98% | 44.66% | 43.44% | 5 | 91/190 | 0/40 | 0 | 5 |
| support | independent | 195/246 | 79.27% | 27.12% | 25.62% | 3 | 48/190 | 0/40 | 126 | 50 |
| conflicts_and_support | independent | 195/246 | 79.27% | 27.12% | 25.62% | 3 | 48/190 | 0/40 | 126 | 50 |
| default | one_to_one | 313/414 | 75.60% | 43.56% | 43.12% | 5 | 96/190 | 0/40 | 0 | 0 |
| conflicts | one_to_one | 313/409 | 76.53% | 43.56% | 42.60% | 5 | 91/190 | 0/40 | 0 | 5 |
| support | one_to_one | 189/240 | 78.75% | 26.30% | 25.00% | 3 | 48/190 | 0/40 | 124 | 50 |
| conflicts_and_support | one_to_one | 189/240 | 78.75% | 26.30% | 25.00% | 3 | 48/190 | 0/40 | 124 | 50 |

Unique coverage counts selections on uniquely labeled fields, including wrong targets. Overall coverage counts all proposals / 960. Sample-support policies trade useful matches for abstention and cannot detect hidden scopes with identical observations. Empty proposals would have undefined precision. Defaults remain unchanged. These are descriptive synthetic development results, not independent validation.
