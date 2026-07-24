#!/usr/bin/env python3
import argparse
import gzip
import re
from collections import Counter
from pathlib import Path

LOG_RE = re.compile(
    r'(?P<ip>\S+) \S+ \S+ \[(?P<time>[^\]]+)\] "(?P<method>\S+) (?P<path>\S+) (?P<proto>[^"]+)" '
    r'(?P<status>\d+) (?P<size>\S+) "(?P<referrer>[^"]*)" "(?P<ua>[^"]*)"'
)


def iter_lines(paths):
    for path in paths:
        p = Path(path)
        if not p.exists():
            continue
        opener = gzip.open if p.suffix == ".gz" else open
        with opener(p, "rt", errors="ignore") as handle:
            for line in handle:
                yield line


def is_noise(path, ua):
    static_ext = (".css", ".js", ".svg", ".png", ".webp", ".ico", ".jpg", ".jpeg", ".gif", ".map")
    if path.split("?", 1)[0].endswith(static_ext):
        return True
    lowered = ua.lower()
    return any(bot in lowered for bot in ("bot", "spider", "crawler", "preview", "monitor"))


def main():
    parser = argparse.ArgumentParser(description="Summarize Cloud Waste Scanner nginx access logs.")
    parser.add_argument("logs", nargs="+", help="Access log files, including rotated .gz files.")
    parser.add_argument("--include-bots", action="store_true", help="Include obvious bots and static assets.")
    parser.add_argument("--top", type=int, default=20)
    args = parser.parse_args()

    ips = Counter()
    pages = Counter()
    referrers = Counter()
    agents = Counter()
    downloads = Counter()
    statuses = Counter()
    total = 0

    for line in iter_lines(args.logs):
        match = LOG_RE.match(line)
        if not match:
            continue
        row = match.groupdict()
        path = row["path"]
        ua = row["ua"]
        if not args.include_bots and is_noise(path, ua):
            continue
        total += 1
        ips[row["ip"]] += 1
        pages[path.split("?", 1)[0]] += 1
        statuses[row["status"]] += 1
        if row["referrer"] and row["referrer"] != "-":
          referrers[row["referrer"]] += 1
        agents[ua] += 1
        if "/download/files/" in path or "/downloads/" in path:
            downloads[path.split("?", 1)[0]] += 1

    def print_counter(title, counter):
        print(f"\n{title}")
        for key, count in counter.most_common(args.top):
            print(f"{count:>6}  {key}")

    print(f"requests={total}")
    print(f"unique_ips={len(ips)}")
    print_counter("status", statuses)
    print_counter("top_pages", pages)
    print_counter("downloads", downloads)
    print_counter("top_ips", ips)
    print_counter("referrers", referrers)
    print_counter("user_agents", agents)


if __name__ == "__main__":
    main()
