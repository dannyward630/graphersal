#!/bin/bash
# Publishes the mirror on GitHub Pages, for the countries that block the droplet's addresses:
#
#   https://graphersal.github.io/        the book            (primary: https://graphersal.dev/)
#   https://graphersal.github.io/play/   the playground      (primary: https://play.graphersal.dev/)
#
# Takes the site deploy/build-site.sh built (target/site/), rewrites the links between the book and
# the playground to the mirror's addresses (the book's run buttons and playground links, the
# playground's link to the book), and pushes it to github.com/graphersal/graphersal.github.io as
# ONE commit that replaces the previous one (the repository keeps no history of old playground
# versions). Everything inside the playground is relative, so it works under /play/ unchanged.
# The schema identifier https://graphersal.dev/schema/v1 is a name, not a link: it stays.
#
#   deploy/build-site.sh && deploy/mirror.sh
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
site="$root/target/site"
out="$root/target/mirror"
repo="git@github.com:graphersal/graphersal.github.io.git"
mirror="https://graphersal.github.io/"

[[ -f "$site/play/VERSION" ]] || { echo "mirror: build the site first (deploy/build-site.sh)" >&2; exit 1; }
version=$(cat "$site/play/VERSION")

rm -rf "$out"
mkdir -p "$out"
cp -R "$site/book/." "$out/"
cp -R "$site/play" "$out/play"
# GitHub Pages serves the files as they are (no Jekyll).
touch "$out/.nojekyll"

# The book's links into the playground, and the playground's link to the book.
grep -rlF "https://play.graphersal.dev/" "$out" --include='*.html' --include='*.js' \
  | while read -r f; do sed -i '' -e "s#https://play.graphersal.dev/#${mirror}play/#g" "$f"; done
config="$out/play/v/$version/js/config.js"
grep -q "book: 'https://graphersal.dev/'" "$config" \
  || { echo "mirror: the playground's book link is not where it was ($config)" >&2; exit 1; }
sed -i '' -e "s#book: 'https://graphersal.dev/'#book: '${mirror}'#" "$config"

# What still points at the primary site (the schema identifier, the dev server's share target):
left=$(grep -rhoF "graphersal.dev" "$out" --include='*.html' --include='*.js' | wc -l | tr -d ' ')
echo "mirror: built $out (playground version $version; $left mentions of graphersal.dev left)"

git -C "$out" init -q -b main
git -C "$out" add -A
git -C "$out" -c user.name="Aleksandr Dederer" -c user.email="aleks@graphersal.dev" \
  commit -q -s -m "Mirror of graphersal.dev, playground version $version"
git -C "$out" push -q --force "$repo" main
echo "mirror: published ${mirror} (playground: ${mirror}play/)"
