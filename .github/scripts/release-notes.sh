#!/usr/bin/env bash
# Print the GitHub release notes for <version>: its section of CHANGELOG.md, then the
# install instructions. Fails when CHANGELOG.md has no "## <version> (date)" section.
#
# Usage: release-notes.sh <version>            (CHANGELOG env var overrides the file)
#        release-notes.sh --check <version>    (only check that the section exists)
set -euo pipefail

check_only=false
if [[ "${1:-}" == "--check" ]]; then check_only=true; shift; fi
version="${1:?usage: release-notes.sh [--check] <version>}"
changelog="${CHANGELOG:-CHANGELOG.md}"

# Lines between "## <version> ..." and the next "## " heading, without the heading.
section=$(awk -v v="$version" '
  index($0, "## " v " ") == 1 || $0 == "## " v { found = 1; on = 1; next }
  /^## / { on = 0 }
  on { print }
  END { exit found ? 0 : 1 }
' "$changelog") || {
  echo "::error::CHANGELOG.md has no \"## ${version} (date)\" section. Add the changes for this version first." >&2
  exit 1
}
# Trim leading and trailing blank lines.
section=$(printf '%s\n' "$section" | sed -e '/./,$!d')
section="${section%"${section##*[![:space:]]}"}"

if [[ -z "$section" ]]; then
  echo "::error::The ${version} section of CHANGELOG.md is empty." >&2
  exit 1
fi
$check_only && exit 0

cat <<NOTES
${section}

---

**New install:** download the \`_x64-setup.exe\` file below and run it. It installs for your user only and needs no administrator rights.
The installer isn't code-signed, so Windows SmartScreen warns the first time: click **More info → Run anyway**.

**Already installed:** TubeDeck shows "Update available" in the bottom-left corner. Click it to see what's new and update.
NOTES
