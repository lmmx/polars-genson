
c0  (n per variant: [19, 19])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 4.19 | 9.51 | 2.90 | 2.90 | 2.66 | 1.49 | 1,749 | 54.09 | 3.77–5.38 |
| Y | 5.07 | 9.50 | 5.70 | 3.76 | 5.39 | 1.59 | 1,674 | 54.09 | 4.57–6.15 |

r10  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.47 | 1.11 | 0.14 | 0.32 | 0.08 | 0.13 | 421 | 6.17 | 0.44–0.48 |
| Y | 0.46 | 1.11 | 0.12 | 0.32 | 0.07 | 0.13 | 431 | 6.17 | 0.41–0.51 |

s10  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.48 | 1.15 | 0.15 | 0.36 | 0.10 | 0.13 | 500 | 4.34 | 0.41–0.54 |
| Y | 0.42 | 1.01 | 0.15 | 0.30 | 0.10 | 0.12 | 508 | 4.34 | 0.41–0.50 |

r30  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.24 | 0.42 | 0.08 | 0.12 | 0.03 | 0.05 | 249 | 1.97 | 0.21–0.32 |
| Y | 0.26 | 0.47 | 0.08 | 0.14 | 0.03 | 0.06 | 246 | 1.97 | 0.21–0.32 |

s30  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 1.57 | 3.05 | 1.56 | 1.36 | 1.47 | 0.71 | 1,016 | 13.87 | 1.13–1.91 |
| Y | 1.77 | 3.20 | 2.45 | 1.60 | 2.36 | 0.45 | 1,006 | 13.87 | 1.63–2.49 |

r50  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.32 | 0.59 | 0.12 | 0.17 | 0.05 | 0.07 | 302 | 3.12 | 0.29–0.34 |
| Y | 0.36 | 0.65 | 0.14 | 0.19 | 0.04 | 0.07 | 299 | 3.12 | 0.33–0.40 |

s50  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.28 | 0.54 | 0.09 | 0.15 | 0.04 | 0.07 | 320 | 2.67 | 0.26–0.29 |
| Y | 0.28 | 0.54 | 0.10 | 0.16 | 0.04 | 0.07 | 315 | 2.67 | 0.25–0.32 |

r70  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.21 | 0.37 | 0.08 | 0.10 | 0.02 | 0.04 | 245 | 1.71 | 0.19–0.25 |
| Y | 0.22 | 0.39 | 0.08 | 0.11 | 0.04 | 0.05 | 241 | 1.71 | 0.19–0.25 |

s70  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 1.04 | 2.07 | 1.13 | 0.84 | 1.03 | 0.50 | 715 | 8.18 | 0.82–1.22 |
| Y | 1.34 | 2.01 | 1.87 | 1.18 | 1.79 | 0.60 | 717 | 8.18 | 1.03–1.53 |

r90  (n per variant: [7, 7])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 0.15 | 0.18 | 0.07 | 0.04 | 0.02 | 0.02 | 182 | 0.48 | 0.14–0.17 |
| Y | 0.19 | 0.17 | 0.08 | 0.05 | 0.01 | 0.02 | 179 | 0.48 | 0.14–0.22 |

s90  (n per variant: [19, 19])
| variant | process wall | process user | process sys | claims wall | claims sys | write stage | peak RSS MB | claims MB | wall range |
|---|---|---|---|---|---|---|---|---|---|
| B | 1.47 | 3.66 | 1.02 | 1.22 | 0.91 | 0.41 | 1,218 | 18.77 | 1.33–1.56 |
| Y | 1.30 | 3.63 | 0.40 | 1.06 | 0.32 | 0.42 | 1,169 | 18.77 | 1.19–3.07 |

c0: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 19 | +0.89 (+0.71, +1.08) | +0.07 (-0.11, +0.21) | +2.91 (+2.43, +3.25) | +0.11 (+0.08, +0.14) |

r10: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | -0.01 (-0.05, +0.05) | +0.00 (-0.12, +0.11) | -0.02 (-0.03, +0.02) | -0.00 (-0.02, +0.01) |

s10: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | -0.06 (-0.08, +0.03) | -0.04 (-0.18, -0.01) | -0.01 (-0.03, +0.02) | -0.00 (-0.04, +0.00) |

r30: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.01 (+0.01, +0.03) | +0.05 (+0.01, +0.07) | +0.00 (-0.02, +0.02) | +0.00 (+0.00, +0.01) |

s30: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.23 (+0.01, +0.78) | +0.14 (-0.18, +0.39) | +0.62 (-0.25, +1.95) | -0.27 (-0.42, +0.14) |

r50: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.04 (+0.03, +0.06) | +0.04 (-0.01, +0.09) | +0.01 (+0.00, +0.06) | +0.01 (-0.00, +0.01) |

s50: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | -0.01 (-0.02, +0.00) | -0.02 (-0.05, +0.01) | +0.00 (-0.01, +0.02) | -0.00 (-0.01, +0.00) |

r70: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.00 (-0.01, +0.02) | +0.00 (-0.01, +0.04) | +0.01 (+0.00, +0.01) | +0.00 (+0.00, +0.01) |

s70: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.38 (+0.10, +0.52) | -0.02 (-0.18, +0.11) | +1.17 (+0.46, +1.53) | +0.09 (+0.04, +0.18) |

r90: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 7 | +0.05 (-0.01, +0.06) | +0.01 (-0.03, +0.06) | +0.02 (+0.00, +0.05) | +0.00 (-0.00, +0.02) |

s90: paired differences from B (median over rounds, 90% bootstrap interval), seconds
| variant | rounds | process wall | process user | process sys | write stage |
|---|---|---|---|---|---|
| Y | 19 | -0.11 (-0.21, +0.93) | -0.03 (-0.10, +0.15) | -0.59 (-0.66, +3.33) | +0.03 (+0.00, +0.40) |

Paired (Y - B) medians per chunk: process wall, claims user CPU, claims sys, stages (s), peak RSS (MB)
| chunk | rounds | B wall | process wall | claims user | claims sys | extract | infer | decode | write | peak RSS |
|---|---|---|---|---|---|---|---|---|---|---|
| c0 | 19 | 4.19 | +0.89 | -0.13 | +2.87 | -0.117 | +0.061 | +0.436 | +0.107 | -75 |
| r10 | 7 | 0.47 | -0.01 | +0.00 | -0.02 | -0.010 | -0.001 | +0.006 | -0.001 | +11 |
| s10 | 7 | 0.48 | -0.06 | -0.09 | +0.00 | -0.007 | +0.005 | -0.007 | -0.004 | +5 |
| r30 | 7 | 0.24 | +0.01 | +0.04 | +0.01 | +0.000 | +0.004 | +0.006 | +0.003 | -3 |
| s30 | 7 | 1.57 | +0.23 | +0.15 | +0.62 | -0.018 | +0.045 | +0.270 | -0.271 | -17 |
| r50 | 7 | 0.32 | +0.04 | +0.02 | -0.01 | -0.001 | +0.005 | +0.003 | +0.007 | -2 |
| s50 | 7 | 0.28 | -0.01 | -0.03 | +0.01 | -0.004 | +0.002 | -0.001 | -0.003 | -6 |
| r70 | 7 | 0.21 | +0.00 | +0.00 | +0.01 | -0.001 | +0.002 | +0.001 | +0.004 | -2 |
| s70 | 7 | 1.04 | +0.38 | -0.02 | +1.21 | -0.025 | +0.010 | +0.013 | +0.090 | +3 |
| r90 | 7 | 0.15 | +0.05 | +0.02 | -0.01 | -0.001 | -0.001 | -0.001 | +0.002 | -2 |
| s90 | 19 | 1.47 | -0.11 | -0.05 | -0.60 | -0.035 | +0.017 | -0.007 | +0.031 | -56 |
sum of median process wall: B 10.41 s, Y 11.66 s
