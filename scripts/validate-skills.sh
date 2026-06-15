#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT_DIR/skills/manifest.json"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

[[ -f "$MANIFEST" ]] || fail "missing skills/manifest.json"

python3 - "$ROOT_DIR" <<'PY'
import json
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1])
manifest_path = root / "skills" / "manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

required_manifest_keys = {"schema_name", "schema_version", "skills", "platforms", "examples"}
missing = required_manifest_keys - set(manifest)
if missing:
    raise SystemExit(f"manifest missing keys: {sorted(missing)}")

if manifest["schema_name"] != "cws_skills_manifest":
    raise SystemExit("manifest schema_name must be cws_skills_manifest")

skills = manifest.get("skills", [])
if not skills:
    raise SystemExit("manifest contains no skills")

name_re = re.compile(r"^[a-z0-9][a-z0-9-]{0,62}[a-z0-9]$")
for item in skills:
    name = item.get("name", "")
    if not name_re.match(name):
        raise SystemExit(f"invalid skill name: {name!r}")
    rel_path = item.get("path")
    if not rel_path:
        raise SystemExit(f"skill {name} missing path")
    skill_dir = root / rel_path
    skill_md = skill_dir / "SKILL.md"
    if not skill_md.exists():
        raise SystemExit(f"skill {name} missing SKILL.md")
    text = skill_md.read_text(encoding="utf-8")
    if not text.startswith("---\n"):
        raise SystemExit(f"skill {name} missing YAML frontmatter")
    try:
        _, frontmatter, body = text.split("---", 2)
    except ValueError:
        raise SystemExit(f"skill {name} has malformed YAML frontmatter")
    if f"name: {name}" not in frontmatter:
        raise SystemExit(f"skill {name} frontmatter name mismatch")
    desc_match = re.search(r"^description:\s*(.+)$", frontmatter, re.M)
    if not desc_match or len(desc_match.group(1).strip()) < 40:
        raise SystemExit(f"skill {name} description missing or too short")
    if len(body.strip()) < 200:
        raise SystemExit(f"skill {name} body is too short")

    for platform in item.get("platforms", []):
        if platform not in manifest.get("platforms", {}):
            raise SystemExit(f"skill {name} references unknown platform {platform}")

    for script_rel in item.get("scripts", []):
        script_path = skill_dir / script_rel
        if not script_path.exists():
            raise SystemExit(f"skill {name} missing script {script_rel}")

    refs = re.findall(r"`(references/[^`]+)`", text)
    for ref in refs:
        ref_path = skill_dir / ref
        if not ref_path.exists():
            raise SystemExit(f"skill {name} references missing file {ref}")

for key, rel_path in manifest.get("examples", {}).items():
    example_path = root / rel_path
    if not example_path.exists():
        raise SystemExit(f"missing example {key}: {rel_path}")

print(f"Validated {len(skills)} skills from {manifest_path.relative_to(root)}")
PY
