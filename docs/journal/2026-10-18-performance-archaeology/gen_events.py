"""A demanding non-Wikidata workload: event-log rows in the shape of public API event
feeds (each event type with its own payload, optional and null fields, nested lists of
records, integers and floats in one field, escaped and non-ASCII strings, timestamps), as
newline-delimited JSON. Deterministic for a given seed.

Usage: python gen_events.py <rows> <out.ndjson> [seed]
"""

import json
import random
import sys

rows, out = int(sys.argv[1]), sys.argv[2]
rng = random.Random(int(sys.argv[3]) if len(sys.argv) > 3 else 7)
WORDS = ["fix", "add", "café", "naïve", "日本", "emoji 😀", 'quote "x"', "tab\there", "line\nbreak", "path\\to"]


def text(n=3):
    return " ".join(rng.choice(WORDS) for _ in range(rng.randint(1, n)))


def user():
    u = {"id": rng.randint(1, 10**7), "login": f"user{rng.randint(1, 5000)}", "site_admin": rng.random() < 0.01}
    if rng.random() < 0.3:
        u["display_name"] = text(2)
    if rng.random() < 0.05:
        u["display_name"] = None
    return u


def label():
    return {"name": rng.choice(["bug", "docs", "perf", "good first issue"]), "color": f"{rng.randint(0, 0xFFFFFF):06x}"}


def payload(kind):
    if kind == "PushEvent":
        return {
            "ref": f"refs/heads/{rng.choice(['main', 'dev', 'feature'])}",
            "size": rng.randint(0, 20),
            "commits": [
                {"sha": f"{rng.getrandbits(160):040x}", "message": text(6), "author": {"name": text(1), "email": "a@b.c"}, "distinct": rng.random() < 0.9}
                for _ in range(rng.randint(0, 6))
            ],
        }
    if kind in ("IssuesEvent", "IssueCommentEvent"):
        issue = {
            "number": rng.randint(1, 30000),
            "title": text(5),
            "state": rng.choice(["open", "closed"]),
            "labels": [label() for _ in range(rng.randint(0, 3))],
            "user": user(),
            "comments": rng.randint(0, 200),
            "score": rng.choice([rng.randint(0, 100), round(rng.random() * 100, 3)]),
            "closed_at": None if rng.random() < 0.6 else "2026-01-02T03:04:05Z",
        }
        p = {"action": rng.choice(["opened", "closed", "created", "edited"]), "issue": issue}
        if kind == "IssueCommentEvent":
            p["comment"] = {"id": rng.randint(1, 10**9), "body": text(12), "user": user(), "reactions": {"+1": rng.randint(0, 9), "heart": rng.randint(0, 3)}}
        return p
    if kind == "WatchEvent":
        return {"action": "started"}
    if kind == "ReleaseEvent":
        return {"action": "published", "release": {"tag_name": f"v{rng.randint(0, 9)}.{rng.randint(0, 20)}", "prerelease": rng.random() < 0.2, "assets": [{"name": f"pkg{i}.whl", "size": rng.randint(1, 10**8), "download_count": rng.randint(0, 10**5)} for i in range(rng.randint(0, 4))]}}
    return {"ref_type": rng.choice(["branch", "tag"]), "ref": text(1) if rng.random() < 0.9 else None, "master_branch": "main"}


KINDS = ["PushEvent"] * 5 + ["IssuesEvent"] * 2 + ["IssueCommentEvent"] * 2 + ["WatchEvent"] * 3 + ["ReleaseEvent", "CreateEvent", "DeleteEvent"]
with open(out, "w") as f:
    for i in range(rows):
        kind = rng.choice(KINDS)
        event = {
            "id": str(10**10 + i),
            "type": kind,
            "actor": user(),
            "repo": {"id": rng.randint(1, 10**8), "name": f"org{rng.randint(1, 300)}/repo{rng.randint(1, 3000)}"},
            "payload": payload(kind),
            "public": rng.random() < 0.97,
            "created_at": f"2026-{rng.randint(1, 12):02d}-{rng.randint(1, 28):02d}T{rng.randint(0, 23):02d}:00:00Z",
        }
        if rng.random() < 0.4:
            event["org"] = {"id": rng.randint(1, 10**6), "login": f"org{rng.randint(1, 300)}"}
        f.write(json.dumps(event, ensure_ascii=rng.random() < 0.5) + "\n")
