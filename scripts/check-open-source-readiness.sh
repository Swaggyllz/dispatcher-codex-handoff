#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

required_files=(
  README.md
  LICENSE
  NOTICE
  CHANGELOG.md
  CONTRIBUTING.md
  SECURITY.md
  CODE_OF_CONDUCT.md
)

for file in "${required_files[@]}"; do
  test -s "$file" || {
    echo "Missing required public file: $file" >&2
    exit 1
  }
done

if git ls-files \
  | grep -Ev '(^|/)\.env\.example$' \
  | grep -Eq '(^|/)(\.env($|\.)|auth\.json$|.*\.(db|sqlite|sqlite3)$)'; then
  echo "Tracked credential or runtime data file detected." >&2
  exit 1
fi

if git grep -nEi 'farion1231|src-tauri|cc-switch|ccswitch' -- \
  ':!NOTICE' ':!scripts/check-open-source-readiness.sh'; then
  echo "Inherited publication metadata remains in tracked files." >&2
  exit 1
fi

if git grep -nEi '/Users/[^/]+/|/home/[^/]+/|@local' -- \
  ':!scripts/check-open-source-readiness.sh'; then
  echo "Personal identity or local filesystem path detected." >&2
  exit 1
fi

if git grep -nE 'sk-[A-Za-z0-9_-]{24,}|AIza[0-9A-Za-z_-]{30,}' -- \
  ':!.env.example' ':!**/*test*' ':!docs/**'; then
  echo "Possible committed API credential detected." >&2
  exit 1
fi

echo "Open-source repository checks passed."
