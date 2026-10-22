c0 Z: 7 files, all byte-identical to B
s90 Z: 7 files, all byte-identical to B

c0  (n per variant: [15, 15])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 4.66 | 9.65 | 3.95 | 3.35 | 3.73 | 1.84 | 1,742 | 54.09 | 4.04–6.63 |
| Z | 4.86 | 9.14 | 5.27 | 3.47 | 5.08 | 1.17 | 1,657 | 54.09 | 3.52–6.82 |

s90  (n per variant: [15, 15])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 1.39 | 3.63 | 0.47 | 1.13 | 0.34 | 0.38 | 1,243 | 18.77 | 1.18–1.82 |
| Z | 1.44 | 3.43 | 1.02 | 1.23 | 0.96 | 0.34 | 1,163 | 18.77 | 1.15–1.77 |

c0: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Z | 15 | +0.47 (-0.85, +0.70) | -0.43 (-0.61, -0.27) | +1.76 (-3.34, +2.77) | -0.57 (-0.76, -0.41) |

s90: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Z | 15 | +0.05 (-0.29, +0.24) | -0.26 (-0.51, -0.12) | +0.58 (-0.77, +0.89) | -0.05 (-0.06, -0.03) |
