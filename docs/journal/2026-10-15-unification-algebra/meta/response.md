Two additions beyond what the chat covered, both in the brief:

- **WP0 snapshot corpus** — the acceptance oracle for every output-preserving package. Without it the agents can't tell a refactor from a behaviour change.
- **WP10 shape fingerprint before building** — hashing rows on the simd-json tape and only building builders for novel shapes. It's the one thing I'd expect to beat interning on speed for Wikidata-style inputs, and it splits cleanly into two agents.

For right now, at 97%: give the agent **WP0, WP2, WP3** (all S, all mechanical, all output-preserving) and optionally **WP13** (prose, no code risk). Hold WP1 and everything from WP4 on until after reset — WP6 and WP8 are the two large serial ones and shouldn't be started on a thin quota.

The brief's "What not to do" section exists so an agent doesn't take files 02–06 of the journal as a spec: no search, no backtracking, no orbital graphs — that framing was corrected in file 08 and the brief encodes the corrected version.