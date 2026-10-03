#!/usr/bin/env python3
"""Convert legacy combined logs into CWS JSON analytics records.

Legacy combined logs do not contain the virtual host. To limit false matches,
this tool only emits successful GETs whose paths resolve to files in the CWS
web root. Emitted records are explicitly marked as host-inferred.
"""

import argparse
import gzip
import json
import re
from datetime import datetime
from pathlib import Path
from urllib.parse import unquote, urlsplit


LOG_RE = re.compile(
    r'(?P<ip>\S+) \S+ \S+ \[(?P<time>[^\]]+)\] '
    r'"(?P<method>\S+) (?P<uri>\S+) (?P<proto>[^"]+)" '
    r'(?P<status>\d+) (?P<size>\S+) "(?P<referrer>[^"]*)" "(?P<ua>[^"]*)"'
)


def iter_lines(paths):
    for raw_path in paths:
        path = Path(raw_path)
        opener = gzip.open if path.suffix == ".gz" else open
        with opener(path, "rt", encoding="utf-8", errors="replace") as handle:
            yield from handle


def resolves_to_cws_file(web_root, request_path):
    path = unquote(request_path)
    if path == "/":
        candidate = web_root / "index.html"
    elif path in ("/blog", "/blog/"):
        candidate = web_root / "blog.html"
    else:
        candidate = web_root / path.lstrip("/")
        if path.endswith("/"):
            candidate = candidate / "index.html"

    try:
        candidate = candidate.resolve()
        candidate.relative_to(web_root)
    except (OSError, ValueError):
        return False
    return candidate.is_file()


def convert(lines, web_root, before):
    for line in lines:
        match = LOG_RE.match(line)
        if not match:
            continue
        row = match.groupdict()
        if row["method"] != "GET" or int(row["status"]) not in (200, 304):
            continue
        timestamp = datetime.strptime(row["time"], "%d/%b/%Y:%H:%M:%S %z").timestamp()
        if before is not None and timestamp >= before:
            continue
        request_path = urlsplit(row["uri"]).path
        if not resolves_to_cws_file(web_root, request_path):
            continue
        yield {
            "ts": timestamp,
            "ip": row["ip"],
            "host": "cloud-waste-scanner.com",
            "host_inferred": True,
            "method": row["method"],
            "uri": row["uri"],
            "status": int(row["status"]),
            "bytes": 0 if row["size"] == "-" else int(row["size"]),
            "ref": row["referrer"],
            "ua": row["ua"],
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", help="Combined access logs, including .gz files")
    parser.add_argument("--web-root", required=True, type=Path)
    parser.add_argument("--before", type=float, help="Exclude records at or after this Unix timestamp")
    args = parser.parse_args()

    web_root = args.web_root.resolve()
    for entry in convert(iter_lines(args.logs), web_root, args.before):
        print(json.dumps(entry, ensure_ascii=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
