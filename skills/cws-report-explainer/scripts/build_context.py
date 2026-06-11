#!/usr/bin/env python3
"""Build a normalized context bundle for the cws-report-explainer skill."""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a normalized CWS context bundle from local API or exported files."
    )
    parser.add_argument("--base-url", help="CWS local API base URL, for example http://127.0.0.1:43177")
    parser.add_argument("--token", help="Bearer token for the local API")
    parser.add_argument(
        "--input",
        action="append",
        default=[],
        help="Input export file path. May be passed multiple times.",
    )
    parser.add_argument(
        "--output",
        help="Optional output file path. Defaults to stdout.",
    )
    return parser.parse_args()


def load_api_client():
    repo_root = Path(__file__).resolve().parents[3]
    sdk_root = repo_root / "sdks" / "python"
    sys.path.insert(0, str(sdk_root))
    from cws_client import CwsClient  # type: ignore

    return CwsClient


def unwrap_envelope(payload: Any) -> list[dict[str, Any]]:
    if isinstance(payload, dict):
        for key in ("items", "data", "results", "findings", "reports", "scans"):
            value = payload.get(key)
            if isinstance(value, list):
                return [row for row in value if isinstance(row, dict)]
    if isinstance(payload, list):
        return [row for row in payload if isinstance(row, dict)]
    return []


def to_number(value: Any) -> float:
    if isinstance(value, (int, float)):
        return float(value)
    if isinstance(value, str):
        try:
            return float(value.replace(",", "").strip())
        except ValueError:
            return 0.0
    return 0.0


def first_present(row: dict[str, Any], keys: list[str], default: str = "") -> str:
    for key in keys:
        value = row.get(key)
        if value is None:
            continue
        text = str(value).strip()
        if text:
            return text
    return default


def summarize_findings(findings: list[dict[str, Any]]) -> dict[str, Any]:
    provider_totals: dict[str, float] = defaultdict(float)
    resource_type_totals: dict[str, float] = defaultdict(float)
    account_totals: dict[str, float] = defaultdict(float)
    currencies: set[str] = set()
    total = 0.0

    for row in findings:
        amount = to_number(
            row.get("estimated_monthly_waste")
            or row.get("estimated_waste")
            or row.get("monthly_waste")
            or row.get("savings_monthly")
            or row.get("cost_monthly")
        )
        currency = first_present(row, ["currency", "estimated_monthly_waste_currency"], "USD")
        provider = first_present(row, ["provider", "cloud_provider"], "unknown")
        resource_type = first_present(row, ["resource_type", "type"], "unknown")
        account = first_present(row, ["account_name", "account_id", "subscription_id"], "unknown")
        total += amount
        currencies.add(currency)
        provider_totals[provider] += amount
        resource_type_totals[resource_type] += amount
        account_totals[account] += amount

    def top_rows(source: dict[str, float]) -> list[dict[str, Any]]:
        rows = [{"label": key, "estimated_monthly_waste": round(value, 2)} for key, value in source.items()]
        rows.sort(key=lambda item: item["estimated_monthly_waste"], reverse=True)
        return rows[:5]

    return {
        "findings": len(findings),
        "estimated_monthly_waste": round(total, 2),
        "currencies": sorted(currencies),
        "top_providers": top_rows(provider_totals),
        "top_resource_types": top_rows(resource_type_totals),
        "top_accounts": top_rows(account_totals),
    }


def build_from_api(base_url: str, token: str | None) -> dict[str, Any]:
    CwsClient = load_api_client()
    client = CwsClient(base_url=base_url, token=token)
    status = client.status()
    findings = unwrap_envelope(client.list_findings(limit=200, envelope=True))
    scans = unwrap_envelope(client.list_scans(limit=50, envelope=True))
    reports = unwrap_envelope(client.list_reports(limit=50, envelope=True))
    return {
        "source": "api",
        "generated_at": utc_now(),
        "status": status,
        "summary": summarize_findings(findings),
        "findings": findings,
        "scans": scans,
        "reports": reports,
        "notes": [],
    }


def parse_json_file(path: Path) -> tuple[list[dict[str, Any]], list[str]]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict):
        rows = unwrap_envelope(payload)
        if rows:
            return rows, []
        return [], [json.dumps(payload, ensure_ascii=True)]
    if isinstance(payload, list):
        rows = [row for row in payload if isinstance(row, dict)]
        notes = [str(row) for row in payload if not isinstance(row, dict)]
        return rows, notes
    return [], [str(payload)]


def parse_csv_file(path: Path) -> list[dict[str, Any]]:
    with path.open("r", encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        return [dict(row) for row in reader]


def build_from_files(paths: list[str]) -> dict[str, Any]:
    findings: list[dict[str, Any]] = []
    reports: list[dict[str, Any]] = []
    notes: list[str] = []

    for raw_path in paths:
        path = Path(raw_path).expanduser().resolve()
        suffix = path.suffix.lower()
        if suffix == ".json":
            rows, json_notes = parse_json_file(path)
            lower_name = path.name.lower()
            if "report" in lower_name:
                reports.extend(rows)
            else:
                findings.extend(rows)
            notes.extend(json_notes)
        elif suffix == ".csv":
            findings.extend(parse_csv_file(path))
        else:
            notes.append(path.read_text(encoding="utf-8"))

    return {
        "source": "files",
        "generated_at": utc_now(),
        "status": {},
        "summary": summarize_findings(findings),
        "findings": findings,
        "scans": [],
        "reports": reports,
        "notes": notes,
    }


def main() -> int:
    args = parse_args()
    if not args.base_url and not args.input:
        print("Provide --base-url or at least one --input file.", file=sys.stderr)
        return 2

    if args.base_url:
        bundle = build_from_api(args.base_url, args.token)
    else:
        bundle = build_from_files(args.input)

    text = json.dumps(bundle, indent=2, ensure_ascii=True)
    if args.output:
        Path(args.output).write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
