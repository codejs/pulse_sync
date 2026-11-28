#!/usr/bin/env bash
# Zips the repository's source into archives/.
#
# Includes every file git would consider part of the project: tracked files
# plus untracked files that aren't ignored. So anything matched by .gitignore
# (target/, node_modules/, web/dist/, .pulse-dev/, ...) is left out, along
# with all other build output and any .git metadata. Uncommitted changes
# are included.
#
# Usage: scripts/archive.sh [-o output.zip]

set -euo pipefail

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cd "$root"

name="pulse-sync"
out=""
while getopts "o:h" opt; do
  case "$opt" in
    o) out="$OPTARG" ;;
    h) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) exit 2 ;;
  esac
done

if [[ -z "$out" ]]; then
  rev="$(git rev-parse --short HEAD 2>/dev/null || echo nohead)"
  [[ -n "$(git status --porcelain)" ]] && rev="$rev-dirty"
  out="archives/$name-$(date +%Y%m%d-%H%M%S)-$rev.zip"
fi
command -v zip >/dev/null || { echo "error: 'zip' is not installed" >&2; exit 1; }

mkdir -p "$(dirname "$out")"
out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
rm -f "$out"

# Stage a symlink so every entry sits under a top-level "pulse-sync/" folder.
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
ln -s "$root" "$stage/$name"

while IFS= read -r -d '' file; do
  # Never include git metadata: .git directories, or the .git files that
  # submodules and worktrees use. (.gitignore etc. are kept.)
  [[ "/$file/" == */.git/* ]] && continue
  # Only regular files: skips tracked files deleted from the working tree,
  # nested repositories git reports as bare directories, and the output itself.
  [[ -f "$file" && "$root/$file" != "$out" ]] || continue
  printf '%s\n' "$name/$file"
done < <(git ls-files -z --cached --others --exclude-standard) \
  | (cd "$stage" && zip -q -X "$out" -@)

files="$(unzip -Z1 "$out" | grep -vc '/$' || true)"
size="$(du -h "$out" | cut -f1 | tr -d ' ')"
echo "wrote ${out#"$root"/} ($files files, $size)"
