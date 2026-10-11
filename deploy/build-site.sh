#!/bin/bash
# Builds the static sites into target/site/ (deploy/deploy.sh uploads them):
#
#   target/site/book/                  the book (mdbook build book)
#   target/site/play/index.html        the playground's entry page, pointing at ONE version
#   target/site/play/reset/            the reset page
#   target/site/play/v/<version>/      everything the page loads (js, css, assets, pkg)
#   target/site/schema/graphersal-schema-v1.json   served at https://graphersal.dev/schema/v1
#
# <version> is a hash of the files under it, so an unchanged playground keeps its version. The
# server caches /v/ for a year ("immutable") and revalidates index.html and reset/ on every load:
# a deploy reaches every browser at its next load, and a cached old module is never mixed with a
# new page. Inside v/<version>/ every reference is relative (modules, the worker and the wasm are
# resolved through import.meta.url), so the tree is copied unchanged.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/target/site"
src="$root/playground"

mdbook build "$root/book"
"$src/build.sh"

rm -rf "$out"
mkdir -p "$out/play" "$out/schema"
cp -R "$root/book/book" "$out/book"
cp "$root/crates/graphersal/src/graph/schema/graphersal-schema-v1.json" "$out/schema/"

# The version: the hash of every file the page loads, with its path.
version=$(cd "$src" && find assets css js pkg -type f ! -name '.*' | LC_ALL=C sort \
  | while read -r f; do printf '%s  ' "$f"; shasum -a 256 "$f" | cut -d' ' -f1; done \
  | shasum -a 256 | cut -c1-12)

mkdir -p "$out/play/v/$version"
for dir in assets css js pkg; do
  cp -R "$src/$dir" "$out/play/v/$version/$dir"
done
cp -R "$src/reset" "$out/play/reset"

# index.html: its three local references move into the version directory; anything else local
# would stay outside the version and is refused.
sed -e "s#href=\"assets/#href=\"v/$version/assets/#g" \
    -e "s#href=\"css/#href=\"v/$version/css/#g" \
    -e "s#src=\"js/#src=\"v/$version/js/#g" \
    "$src/index.html" > "$out/play/index.html"
if grep -nE '(src|href)="[^"#:/][^":]*"' "$out/play/index.html" | grep -v "=\"v/$version/"; then
  echo "build-site: index.html has a local reference outside v/$version/ (above)" >&2
  exit 1
fi

# Files that are not part of the sites: Finder's metadata, the diagram sources of the book.
find "$out" \( -name '.DS_Store' -o -name '*.puml' \) -delete

echo "$version" > "$out/play/VERSION"
echo "built $out (playground version $version)"
