# Release-readiness development exploration

V1 distinctive-context policies preceded their first run. V2 scoped-context policies were developed from V1 development failures. V3 qualified-context refinements and their opt-in runtime port were developed from V2 development failures and useful-match losses, then declared before V3 scoring. Earlier policies remain as comparators. Existing datasets and labels are unchanged. Caller hints and confirmations are excluded. No holdout was scored in this exploration. Evidence derives from observable inputs, never from labels. Runtime selections and rankings are regression-checked against the fixed V3 prototype on every development case. Development success does not establish release readiness; prospective frozen qualification is reported separately.

| Corpus | Policy | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Overall coverage | Candidate recall@5 | Wrong unique | No-match proposals | Ambiguous proposals | Development targets met |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| original_development | default | independent | 321/422 | 76.07% | 43.97% | 44.66% | 43.96% | 100.00% | 5 | 96/190 | 0/40 | false |
| original_development | default | one_to_one | 313/414 | 75.60% | 42.88% | 43.56% | 43.12% | 100.00% | 5 | 96/190 | 0/40 | false |
| original_development | conflicts | independent | 321/417 | 76.98% | 43.97% | 44.66% | 43.44% | 100.00% | 5 | 91/190 | 0/40 | false |
| original_development | conflicts | one_to_one | 313/409 | 76.53% | 42.88% | 43.56% | 42.60% | 100.00% | 5 | 91/190 | 0/40 | false |
| original_development | distinctive_context | independent | 563/566 | 99.47% | 77.12% | 77.12% | 58.96% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | distinctive_context | one_to_one | 558/561 | 99.47% | 76.44% | 76.44% | 58.44% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | distinctive_context_strict_identifiers | independent | 543/543 | 100.00% | 74.38% | 74.38% | 56.56% | 100.00% | 0 | 0/190 | 0/40 | true |
| original_development | distinctive_context_strict_identifiers | one_to_one | 538/538 | 100.00% | 73.70% | 73.70% | 56.04% | 100.00% | 0 | 0/190 | 0/40 | true |
| original_development | scoped_context | independent | 456/459 | 99.35% | 62.47% | 62.47% | 47.81% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | scoped_context | one_to_one | 451/454 | 99.34% | 61.78% | 61.78% | 47.29% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | scoped_context_strict_identifiers | independent | 436/436 | 100.00% | 59.73% | 59.73% | 45.42% | 100.00% | 0 | 0/190 | 0/40 | false |
| original_development | scoped_context_strict_identifiers | one_to_one | 431/431 | 100.00% | 59.04% | 59.04% | 44.90% | 100.00% | 0 | 0/190 | 0/40 | false |
| original_development | qualified_context | independent | 490/493 | 99.39% | 67.12% | 67.12% | 51.35% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | qualified_context | one_to_one | 485/488 | 99.39% | 66.44% | 66.44% | 50.83% | 100.00% | 0 | 3/190 | 0/40 | true |
| original_development | qualified_context_strict_identifiers | independent | 470/470 | 100.00% | 64.38% | 64.38% | 48.96% | 100.00% | 0 | 0/190 | 0/40 | true |
| original_development | qualified_context_strict_identifiers | one_to_one | 465/465 | 100.00% | 63.70% | 63.70% | 48.44% | 100.00% | 0 | 0/190 | 0/40 | true |
| original_development | runtime_context_strict_identifiers | independent | 470/470 | 100.00% | 64.38% | 64.38% | 48.96% | 100.00% | 0 | 0/190 | 0/40 | true |
| original_development | runtime_context_strict_identifiers | one_to_one | 465/465 | 100.00% | 63.70% | 63.70% | 48.44% | 100.00% | 0 | 0/190 | 0/40 | true |
| stage3_extension_without_hints | default | independent | 16/26 | 61.54% | 80.00% | 80.00% | 81.25% | 100.00% | 0 | 10/11 | 0/1 | false |
| stage3_extension_without_hints | default | one_to_one | 15/25 | 60.00% | 75.00% | 75.00% | 78.12% | 100.00% | 0 | 10/11 | 0/1 | false |
| stage3_extension_without_hints | conflicts | independent | 16/26 | 61.54% | 80.00% | 80.00% | 81.25% | 100.00% | 0 | 10/11 | 0/1 | false |
| stage3_extension_without_hints | conflicts | one_to_one | 15/25 | 60.00% | 75.00% | 75.00% | 78.12% | 100.00% | 0 | 10/11 | 0/1 | false |
| stage3_extension_without_hints | distinctive_context | independent | 13/16 | 81.25% | 65.00% | 65.00% | 50.00% | 100.00% | 0 | 3/11 | 0/1 | false |
| stage3_extension_without_hints | distinctive_context | one_to_one | 13/16 | 81.25% | 65.00% | 65.00% | 50.00% | 100.00% | 0 | 3/11 | 0/1 | false |
| stage3_extension_without_hints | distinctive_context_strict_identifiers | independent | 13/16 | 81.25% | 65.00% | 65.00% | 50.00% | 100.00% | 0 | 3/11 | 0/1 | false |
| stage3_extension_without_hints | distinctive_context_strict_identifiers | one_to_one | 13/16 | 81.25% | 65.00% | 65.00% | 50.00% | 100.00% | 0 | 3/11 | 0/1 | false |
| stage3_extension_without_hints | scoped_context | independent | 11/11 | 100.00% | 55.00% | 55.00% | 34.38% | 100.00% | 0 | 0/11 | 0/1 | false |
| stage3_extension_without_hints | scoped_context | one_to_one | 10/10 | 100.00% | 50.00% | 50.00% | 31.25% | 100.00% | 0 | 0/11 | 0/1 | false |
| stage3_extension_without_hints | scoped_context_strict_identifiers | independent | 11/11 | 100.00% | 55.00% | 55.00% | 34.38% | 100.00% | 0 | 0/11 | 0/1 | false |
| stage3_extension_without_hints | scoped_context_strict_identifiers | one_to_one | 10/10 | 100.00% | 50.00% | 50.00% | 31.25% | 100.00% | 0 | 0/11 | 0/1 | false |
| stage3_extension_without_hints | qualified_context | independent | 13/13 | 100.00% | 65.00% | 65.00% | 40.62% | 100.00% | 0 | 0/11 | 0/1 | true |
| stage3_extension_without_hints | qualified_context | one_to_one | 12/12 | 100.00% | 60.00% | 60.00% | 37.50% | 100.00% | 0 | 0/11 | 0/1 | true |
| stage3_extension_without_hints | qualified_context_strict_identifiers | independent | 13/13 | 100.00% | 65.00% | 65.00% | 40.62% | 100.00% | 0 | 0/11 | 0/1 | true |
| stage3_extension_without_hints | qualified_context_strict_identifiers | one_to_one | 12/12 | 100.00% | 60.00% | 60.00% | 37.50% | 100.00% | 0 | 0/11 | 0/1 | true |
| stage3_extension_without_hints | runtime_context_strict_identifiers | independent | 13/13 | 100.00% | 65.00% | 65.00% | 40.62% | 100.00% | 0 | 0/11 | 0/1 | true |
| stage3_extension_without_hints | runtime_context_strict_identifiers | one_to_one | 12/12 | 100.00% | 60.00% | 60.00% | 37.50% | 100.00% | 0 | 0/11 | 0/1 | true |
| corrective_development | default | independent | 20/74 | 27.03% | 14.18% | 14.18% | 34.26% | 100.00% | 0 | 54/63 | 0/12 | false |
| corrective_development | default | one_to_one | 20/74 | 27.03% | 14.18% | 14.18% | 34.26% | 100.00% | 0 | 54/63 | 0/12 | false |
| corrective_development | conflicts | independent | 20/74 | 27.03% | 14.18% | 14.18% | 34.26% | 100.00% | 0 | 54/63 | 0/12 | false |
| corrective_development | conflicts | one_to_one | 20/74 | 27.03% | 14.18% | 14.18% | 34.26% | 100.00% | 0 | 54/63 | 0/12 | false |
| corrective_development | distinctive_context | independent | 112/139 | 80.58% | 79.43% | 79.43% | 64.35% | 100.00% | 0 | 27/63 | 0/12 | false |
| corrective_development | distinctive_context | one_to_one | 112/139 | 80.58% | 79.43% | 79.43% | 64.35% | 100.00% | 0 | 27/63 | 0/12 | false |
| corrective_development | distinctive_context_strict_identifiers | independent | 112/139 | 80.58% | 79.43% | 79.43% | 64.35% | 100.00% | 0 | 27/63 | 0/12 | false |
| corrective_development | distinctive_context_strict_identifiers | one_to_one | 112/139 | 80.58% | 79.43% | 79.43% | 64.35% | 100.00% | 0 | 27/63 | 0/12 | false |
| corrective_development | scoped_context | independent | 87/90 | 96.67% | 61.70% | 61.70% | 41.67% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | scoped_context | one_to_one | 84/87 | 96.55% | 59.57% | 59.57% | 40.28% | 100.00% | 0 | 3/63 | 0/12 | false |
| corrective_development | scoped_context_strict_identifiers | independent | 87/90 | 96.67% | 61.70% | 61.70% | 41.67% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | scoped_context_strict_identifiers | one_to_one | 84/87 | 96.55% | 59.57% | 59.57% | 40.28% | 100.00% | 0 | 3/63 | 0/12 | false |
| corrective_development | qualified_context | independent | 93/96 | 96.88% | 65.96% | 65.96% | 44.44% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | qualified_context | one_to_one | 90/93 | 96.77% | 63.83% | 63.83% | 43.06% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | qualified_context_strict_identifiers | independent | 93/96 | 96.88% | 65.96% | 65.96% | 44.44% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | qualified_context_strict_identifiers | one_to_one | 90/93 | 96.77% | 63.83% | 63.83% | 43.06% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | runtime_context_strict_identifiers | independent | 93/96 | 96.88% | 65.96% | 65.96% | 44.44% | 100.00% | 0 | 3/63 | 0/12 | true |
| corrective_development | runtime_context_strict_identifiers | one_to_one | 90/93 | 96.77% | 63.83% | 63.83% | 43.06% | 100.00% | 0 | 3/63 | 0/12 | true |

Compared with the default on the same corpus and assignment policy:

| Corpus | Policy | Assignment | Preserved correct | Lost correct | New correct | Removed false | New false |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| original_development | default | independent | 321 | 0 | 0 | 0 | 0 |
| original_development | default | one_to_one | 313 | 0 | 0 | 0 | 0 |
| original_development | conflicts | independent | 321 | 0 | 0 | 5 | 0 |
| original_development | conflicts | one_to_one | 313 | 0 | 0 | 5 | 0 |
| original_development | distinctive_context | independent | 306 | 15 | 257 | 98 | 0 |
| original_development | distinctive_context | one_to_one | 301 | 12 | 257 | 98 | 0 |
| original_development | distinctive_context_strict_identifiers | independent | 296 | 25 | 247 | 101 | 0 |
| original_development | distinctive_context_strict_identifiers | one_to_one | 291 | 22 | 247 | 101 | 0 |
| original_development | scoped_context | independent | 255 | 66 | 201 | 98 | 0 |
| original_development | scoped_context | one_to_one | 250 | 63 | 201 | 98 | 0 |
| original_development | scoped_context_strict_identifiers | independent | 245 | 76 | 191 | 101 | 0 |
| original_development | scoped_context_strict_identifiers | one_to_one | 240 | 73 | 191 | 101 | 0 |
| original_development | qualified_context | independent | 289 | 32 | 201 | 98 | 0 |
| original_development | qualified_context | one_to_one | 284 | 29 | 201 | 98 | 0 |
| original_development | qualified_context_strict_identifiers | independent | 279 | 42 | 191 | 101 | 0 |
| original_development | qualified_context_strict_identifiers | one_to_one | 274 | 39 | 191 | 101 | 0 |
| original_development | runtime_context_strict_identifiers | independent | 279 | 42 | 191 | 101 | 0 |
| original_development | runtime_context_strict_identifiers | one_to_one | 274 | 39 | 191 | 101 | 0 |
| stage3_extension_without_hints | default | independent | 16 | 0 | 0 | 0 | 0 |
| stage3_extension_without_hints | default | one_to_one | 15 | 0 | 0 | 0 | 0 |
| stage3_extension_without_hints | conflicts | independent | 16 | 0 | 0 | 0 | 0 |
| stage3_extension_without_hints | conflicts | one_to_one | 15 | 0 | 0 | 0 | 0 |
| stage3_extension_without_hints | distinctive_context | independent | 13 | 3 | 0 | 7 | 0 |
| stage3_extension_without_hints | distinctive_context | one_to_one | 13 | 2 | 0 | 7 | 0 |
| stage3_extension_without_hints | distinctive_context_strict_identifiers | independent | 13 | 3 | 0 | 7 | 0 |
| stage3_extension_without_hints | distinctive_context_strict_identifiers | one_to_one | 13 | 2 | 0 | 7 | 0 |
| stage3_extension_without_hints | scoped_context | independent | 11 | 5 | 0 | 10 | 0 |
| stage3_extension_without_hints | scoped_context | one_to_one | 10 | 5 | 0 | 10 | 0 |
| stage3_extension_without_hints | scoped_context_strict_identifiers | independent | 11 | 5 | 0 | 10 | 0 |
| stage3_extension_without_hints | scoped_context_strict_identifiers | one_to_one | 10 | 5 | 0 | 10 | 0 |
| stage3_extension_without_hints | qualified_context | independent | 13 | 3 | 0 | 10 | 0 |
| stage3_extension_without_hints | qualified_context | one_to_one | 12 | 3 | 0 | 10 | 0 |
| stage3_extension_without_hints | qualified_context_strict_identifiers | independent | 13 | 3 | 0 | 10 | 0 |
| stage3_extension_without_hints | qualified_context_strict_identifiers | one_to_one | 12 | 3 | 0 | 10 | 0 |
| stage3_extension_without_hints | runtime_context_strict_identifiers | independent | 13 | 3 | 0 | 10 | 0 |
| stage3_extension_without_hints | runtime_context_strict_identifiers | one_to_one | 12 | 3 | 0 | 10 | 0 |
| corrective_development | default | independent | 20 | 0 | 0 | 0 | 0 |
| corrective_development | default | one_to_one | 20 | 0 | 0 | 0 | 0 |
| corrective_development | conflicts | independent | 20 | 0 | 0 | 0 | 0 |
| corrective_development | conflicts | one_to_one | 20 | 0 | 0 | 0 | 0 |
| corrective_development | distinctive_context | independent | 20 | 0 | 92 | 30 | 3 |
| corrective_development | distinctive_context | one_to_one | 20 | 0 | 92 | 30 | 3 |
| corrective_development | distinctive_context_strict_identifiers | independent | 20 | 0 | 92 | 30 | 3 |
| corrective_development | distinctive_context_strict_identifiers | one_to_one | 20 | 0 | 92 | 30 | 3 |
| corrective_development | scoped_context | independent | 8 | 12 | 79 | 51 | 0 |
| corrective_development | scoped_context | one_to_one | 8 | 12 | 76 | 51 | 0 |
| corrective_development | scoped_context_strict_identifiers | independent | 8 | 12 | 79 | 51 | 0 |
| corrective_development | scoped_context_strict_identifiers | one_to_one | 8 | 12 | 76 | 51 | 0 |
| corrective_development | qualified_context | independent | 12 | 8 | 81 | 51 | 0 |
| corrective_development | qualified_context | one_to_one | 12 | 8 | 78 | 51 | 0 |
| corrective_development | qualified_context_strict_identifiers | independent | 12 | 8 | 81 | 51 | 0 |
| corrective_development | qualified_context_strict_identifiers | one_to_one | 12 | 8 | 78 | 51 | 0 |
| corrective_development | runtime_context_strict_identifiers | independent | 12 | 8 | 81 | 51 | 0 |
| corrective_development | runtime_context_strict_identifiers | one_to_one | 12 | 8 | 78 | 51 | 0 |

Release decision: **not qualified**. Historical published holdouts cannot be retuned and called fresh.

- Development exploration does not perform frozen prospective qualification; see the separate qualification report
- Synthetic development data alone does not certify production accuracy
