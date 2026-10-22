c0 X0: 7 files, all byte-identical to B
c0 dict32: 7 files, all byte-identical to B
c0 sharerep: 7 files, all byte-identical to B
c0 all: 7 files, all byte-identical to B
c0 page64k: 7 files, 4 differ: ['labels/chunk_0.parquet', 'descriptions/chunk_0.parquet', 'claims/chunk_0.parquet', 'aliases/chunk_0.parquet']
s90 X0: 7 files, all byte-identical to B
s90 dict32: 7 files, all byte-identical to B
s90 sharerep: 7 files, all byte-identical to B
s90 all: 7 files, all byte-identical to B
s90 page64k: 7 files, 4 differ: ['labels/chunk_90.parquet', 'descriptions/chunk_90.parquet', 'claims/chunk_90.parquet', 'aliases/chunk_90.parquet']

c0  (n per variant: [12, 12, 12, 12, 12, 12])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 5.61 | 10.30 | 6.15 | 4.22 | 5.94 | 1.79 | 1,732 | 54.09 | 5.05–7.51 |
| X0 | 4.89 | 10.33 | 4.04 | 3.56 | 3.80 | 1.71 | 1,699 | 54.09 | 4.19–5.49 |
| dict32 | 4.44 | 10.22 | 2.92 | 3.10 | 2.67 | 1.45 | 1,684 | 54.09 | 3.75–4.91 |
| sharerep | 4.59 | 9.79 | 3.41 | 3.18 | 3.16 | 1.36 | 1,666 | 54.09 | 3.91–4.94 |
| all | 4.03 | 9.76 | 2.29 | 2.66 | 2.05 | 1.00 | 1,629 | 54.09 | 3.47–4.53 |
| page64k | 4.15 | 10.25 | 2.16 | 2.75 | 1.91 | 1.15 | 1,660 | 55.98 | 3.61–4.57 |

s90  (n per variant: [12, 12, 12, 12, 12, 12])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 1.39 | 3.89 | 0.41 | 1.15 | 0.31 | 0.43 | 1,208 | 18.77 | 1.21–1.48 |
| X0 | 1.66 | 3.95 | 1.23 | 1.41 | 1.14 | 0.55 | 1,214 | 18.77 | 1.36–1.93 |
| dict32 | 1.43 | 3.89 | 0.46 | 1.19 | 0.34 | 0.46 | 1,178 | 18.77 | 1.28–2.04 |
| sharerep | 1.86 | 3.78 | 1.96 | 1.63 | 1.87 | 0.83 | 1,170 | 18.77 | 1.26–2.29 |
| all | 1.48 | 3.64 | 0.81 | 1.24 | 0.72 | 0.53 | 1,146 | 18.77 | 1.27–1.92 |
| page64k | 1.57 | 3.96 | 1.14 | 1.32 | 1.02 | 0.49 | 1,158 | 19.07 | 1.30–1.78 |

c0: paired differences from X0 (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| B | 12 | +0.68 (+0.44, +1.21) | +0.09 (-0.54, +0.21) | +2.11 (+1.74, +3.62) | +0.05 (-0.05, +0.15) |
| dict32 | 12 | -0.49 (-0.79, -0.18) | -0.18 (-0.44, +0.26) | -1.12 (-1.85, -0.55) | -0.34 (-0.42, -0.15) |
| sharerep | 12 | -0.36 (-0.60, -0.13) | -0.54 (-0.81, -0.27) | -0.19 (-0.92, +0.25) | -0.31 (-0.53, -0.18) |
| all | 12 | -0.75 (-1.31, -0.52) | -0.78 (-1.02, -0.22) | -1.60 (-2.60, -0.89) | -0.70 (-0.81, -0.57) |
| page64k | 12 | -0.95 (-1.03, -0.26) | -0.09 (-0.63, +0.06) | -1.91 (-2.13, -1.05) | -0.60 (-0.68, -0.42) |

s90: paired differences from X0 (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| B | 12 | -0.26 (-0.32, -0.08) | -0.06 (-0.23, -0.01) | -0.84 (-1.13, -0.24) | -0.11 (-0.20, -0.07) |
| dict32 | 12 | -0.08 (-0.29, +0.21) | -0.10 (-0.23, +0.04) | -0.04 (-0.77, +0.38) | -0.06 (-0.18, +0.11) |
| sharerep | 12 | +0.08 (-0.14, +0.54) | -0.33 (-0.39, -0.11) | +0.56 (-0.48, +1.50) | +0.15 (+0.01, +0.35) |
| all | 12 | -0.10 (-0.28, +0.14) | -0.34 (-0.44, -0.20) | -0.18 (-0.51, +0.53) | -0.06 (-0.18, +0.10) |
| page64k | 12 | -0.10 (-0.24, +0.15) | -0.15 (-0.19, +0.11) | -0.19 (-0.56, +0.06) | -0.10 (-0.16, +0.01) |
