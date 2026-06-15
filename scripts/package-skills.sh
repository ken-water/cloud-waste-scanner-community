#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
MANIFEST="$ROOT_DIR/skills/manifest.json"

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

[[ -f "$MANIFEST" ]] || fail "missing skills/manifest.json"

"$ROOT_DIR/scripts/validate-skills.sh"

rm -rf "$DIST_DIR/skills-codex" "$DIST_DIR/skills-claude" "$DIST_DIR/skills-generic"
mkdir -p \
  "$DIST_DIR/skills-codex/.agents/skills" \
  "$DIST_DIR/skills-claude/.claude/skills" \
  "$DIST_DIR/skills-generic/skills"

copy_skill() {
  local src="$1"
  local dest="$2"
  cp -a "$src" "$dest"
  find "$dest" -type d -name '__pycache__' -prune -exec rm -rf {} +
  find "$dest" -type f -name '*.pyc' -delete
}

mapfile -t SKILL_PATHS < <(python3 - "$ROOT_DIR" <<'PY'
import json
import pathlib
import sys
root = pathlib.Path(sys.argv[1])
manifest = json.loads((root / "skills" / "manifest.json").read_text(encoding="utf-8"))
for item in manifest["skills"]:
    print(item["path"])
PY
)

for rel in "${SKILL_PATHS[@]}"; do
  name="$(basename "$rel")"
  copy_skill "$ROOT_DIR/$rel" "$DIST_DIR/skills-codex/.agents/skills/$name"
  copy_skill "$ROOT_DIR/$rel" "$DIST_DIR/skills-claude/.claude/skills/$name"
  rm -rf "$DIST_DIR/skills-claude/.claude/skills/$name/agents"
  copy_skill "$ROOT_DIR/$rel" "$DIST_DIR/skills-generic/skills/$name"
done

cp "$MANIFEST" "$DIST_DIR/skills-codex/skills-manifest.json"
cp "$MANIFEST" "$DIST_DIR/skills-claude/skills-manifest.json"
cp "$MANIFEST" "$DIST_DIR/skills-generic/skills-manifest.json"
cp -a "$ROOT_DIR/skills/examples" "$DIST_DIR/skills-codex/examples"
cp -a "$ROOT_DIR/skills/examples" "$DIST_DIR/skills-claude/examples"
cp -a "$ROOT_DIR/skills/examples" "$DIST_DIR/skills-generic/examples"

for package in skills-codex skills-claude skills-generic; do
  tar -C "$DIST_DIR" -czf "$DIST_DIR/${package}.tar.gz" "$package"
done

echo "Built skill packages:"
echo "- dist/skills-codex.tar.gz"
echo "- dist/skills-claude.tar.gz"
echo "- dist/skills-generic.tar.gz"
