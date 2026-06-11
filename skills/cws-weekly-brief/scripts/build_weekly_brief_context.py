#!/usr/bin/env python3
"""Build a normalized context bundle for the cws-weekly-brief skill."""

from __future__ import annotations

import argparse
import importlib.util
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a normalized CWS weekly brief bundle from local API or exported files."
    )
    parser.add_argument("--base-url", help="CWS local API base URL")
    parser.add_argument("--token", help="Bearer token for the local API")
    parser.add_argument("--current-input", action="append", default=[], help="Current evidence file path")
    parser.add_argument("--previous-input", action="append", default=[], help="Previous evidence file path")
    parser.add_argument("--output", help="Optional output path. Defaults to stdout.")
    return parser.parse_args()


def load_report_explainer_module():
    script_path = Path(__file__).resolve().parents[2] / "cws-report-explainer" / "scripts" / "build_context.py"
    spec = importlib.util.spec_from_file_location("cws_report_explainer_build_context", script_path)
    if spec is None or spec.loader is None:
        raise RuntimeError("failed to load report explainer context builder")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def build_from_api(base_url: str, token: str | None) -> dict[str, Any]:
    helper = load_report_explainer_module()
    current = helper.build_from_api(base_url, token)
    scans = current.get("scans") or []
    previous_summary: dict[str, Any] | None = None
    if len(scans) > 1:
        previous_summary = {
            "findings": scans[1].get("resource_count"),
            "estimated_monthly_waste": scans[1].get("total_waste"),
            "scan_label": scans[1].get("scanned_at"),
        }
    return compose_bundle(current=current, previous_summary=previous_summary, source="api")


def build_from_files(current_inputs: list[str], previous_inputs: list[str]) -> dict[str, Any]:
    helper = load_report_explainer_module()
    current = helper.build_from_files(current_inputs)
    previous = helper.build_from_files(previous_inputs) if previous_inputs else None
    previous_summary = previous.get("summary") if previous else None
    return compose_bundle(current=current, previous_summary=previous_summary, source="files")


def compose_bundle(current: dict[str, Any], previous_summary: dict[str, Any] | None, source: str) -> dict[str, Any]:
    current_summary = current.get("summary") or {}
    current_findings = int(current_summary.get("findings") or 0)
    current_waste = float(current_summary.get("estimated_monthly_waste") or 0.0)
    prev_findings = int(previous_summary.get("findings") or 0) if previous_summary else None
    prev_waste = float(previous_summary.get("estimated_monthly_waste") or 0.0) if previous_summary else None
    delta_findings = current_findings - prev_findings if prev_findings is not None else None
    delta_waste = round(current_waste - prev_waste, 2) if prev_waste is not None else None
    return {
        "source": source,
        "generated_at": utc_now(),
        "current": current,
        "previous": {"summary": previous_summary} if previous_summary else None,
        "delta": {
            "findings": delta_findings,
            "estimated_monthly_waste": delta_waste,
        },
    }


def main() -> int:
    args = parse_args()
    if not args.base_url and not args.current_input:
        print("Provide --base-url or at least one --current-input file path.")
        return 2
    if args.base_url:
        bundle = build_from_api(args.base_url, args.token)
    else:
        bundle = build_from_files(args.current_input, args.previous_input)
    text = json.dumps(bundle, indent=2, ensure_ascii=True)
    if args.output:
        Path(args.output).write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
