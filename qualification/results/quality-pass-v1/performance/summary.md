# Native repeated cost measurements

Five independent processes per revision and mode. Timing uses the normal allocator; separate instrumented processes measure allocation requests. Spread is min–max of process means, without confidence intervals or peak-memory claims.

| Workload | Baseline median µs [min–max] | Candidate median µs [min–max] | Speedup | Allocations baseline → candidate | Allocated bytes baseline → candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| context-128-empty-assignment | 56956.36 [56708.38–59902.46] | 61988.38 [60089.64–79804.24] | 0.92x | 222583 → 229360 | 30051480 → 30523455 |
| context-128-empty-independent | 56440.80 [54443.22–59606.34] | 61619.54 [60704.18–63679.92] | 0.92x | 222192 → 228969 | 29480824 → 29952799 |
| context-128-sampled-assignment | 66069.24 [61801.18–69623.38] | 72643.12 [68882.40–79525.94] | 0.91x | 207479 → 214512 | 37483032 → 37956031 |
| context-128-sampled-independent | 66836.90 [62326.18–67614.68] | 72694.24 [67295.02–73102.26] | 0.92x | 207088 → 214121 | 36912376 → 37385375 |
| context-16-empty-assignment | 752.22 [743.64–760.98] | 967.14 [894.09–971.31] | 0.78x | 4550 → 5503 | 548224 → 599975 |
| context-16-empty-independent | 760.01 [708.94–790.38] | 976.09 [954.85–1016.72] | 0.78x | 4495 → 5448 | 537792 → 589543 |
| context-16-sampled-assignment | 1032.69 [983.02–1164.87] | 1172.06 [1152.48–1219.62] | 0.88x | 4454 → 5439 | 740656 → 792535 |
| context-16-sampled-independent | 1006.03 [990.48–1058.97] | 1191.14 [1161.67–1248.35] | 0.84x | 4399 → 5384 | 730224 → 782103 |
| context-64-empty-assignment | 12165.75 [11818.59–13814.72] | 13333.04 [12775.04–13704.38] | 0.91x | 58054 → 61503 | 7689096 → 7896367 |
| context-64-empty-independent | 11889.75 [11339.20–18257.64] | 12918.83 [12762.39–14168.25] | 0.92x | 57855 → 61304 | 7543016 → 7750287 |
| context-64-sampled-assignment | 15778.42 [15010.48–17510.30] | 16691.21 [15900.37–18729.89] | 0.95x | 54598 → 58175 | 9721416 → 9929199 |
| context-64-sampled-independent | 15613.81 [14547.87–16022.97] | 18082.95 [16036.69–18646.83] | 0.86x | 54399 → 57976 | 9575336 → 9783119 |
| context-ambiguous-assignment | 14072.56 [13684.12–15097.50] | 17106.72 [15607.26–19682.54] | 0.82x | 53355 → 56228 | 9623016 → 9627023 |
| context-ambiguous-independent | 14885.14 [14203.26–19540.62] | 16791.66 [16368.36–19498.68] | 0.89x | 53287 → 56160 | 9554792 → 9558799 |
| context-budget-input-assignment | 0.09 [0.07–0.11] | 0.07 [0.07–0.07] | 1.36x | 1 → 1 | 21 → 21 |
| context-budget-input-independent | 0.09 [0.09–0.11] | 0.07 [0.07–0.07] | 1.37x | 1 → 1 | 21 → 21 |
| context-budget-pairs-assignment | 2.00 [1.42–2.62] | 1.42 [1.38–2.07] | 1.41x | 7 → 7 | 850 → 850 |
| context-budget-pairs-independent | 1.88 [1.44–3.02] | 1.44 [1.39–1.59] | 1.30x | 7 → 7 | 850 → 850 |
| context-budget-report-assignment | 51.97 [43.94–67.42] | 95.87 [93.46–117.75] | 0.54x | 305 → 819 | 52808 → 68808 |
| context-budget-report-independent | 54.31 [50.25–74.39] | 94.27 [91.34–135.62] | 0.58x | 305 → 819 | 52808 → 68808 |
| context-budget-signals-assignment | 2.33 [1.50–3.38] | 1.47 [1.40–1.62] | 1.58x | 7 → 7 | 849 → 849 |
| context-budget-signals-independent | 2.08 [1.63–2.60] | 1.47 [1.43–1.54] | 1.42x | 7 → 7 | 849 → 849 |
| context-disjoint-assignment | 12922.82 [11822.10–14255.44] | 14998.94 [13452.70–22287.32] | 0.86x | 53949 → 58175 | 9570224 → 9929455 |
| context-disjoint-independent | 12467.88 [12111.88–12871.26] | 15010.26 [13780.50–20019.18] | 0.83x | 53881 → 57976 | 9502000 → 9783375 |
