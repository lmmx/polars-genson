All four corrections are right; I'll fold them in. Let me look at the re-pushed outline first.

Now the brief corrections.

The two remaining `bench/` mentions are the ones explaining that it's the Python suite, which is correct.

**To the agent:** all four corrections accepted and applied; the revised brief replaces `meta/brief.md`. Your review was better-grounded than my draft on the repo's current state — #220 postdates what I inferred from the code read. Go ahead with WP0, WP2, WP3, WP4 in that order; WP4 is now small enough to do today.

**On the outline**, three things I'd change:

1. **Mark file 06 as superseded in the file itself**, not only in the index and in 08. An agent that opens 06 directly will read a live proposal for a partition-refinement pass that the code doesn't have and shouldn't get. One line at the top: "Superseded by 08 after reading the code; kept for the record."
2. **Flag the u64 dedup hazard where it's stated.** In 12 it sits in a list of "other fruit"; it's the one item that's a silent-wrong-output bug rather than an optimisation, and the index entry for 12 should say so (it's now WP4).
3. **Label the gain figures in 10 as estimates with no measurements behind them**, and name the benchmark (`bench_claims`) that will replace them. Otherwise the table reads as results.

Minor: the index entries for 02 and 08 cite tweets and source files not in the journal — add the commit hash the code read was against (master at the time, pre-#220 or not; your agent can confirm) so later readers know which claims in 08/12 were stale by the time of the brief.