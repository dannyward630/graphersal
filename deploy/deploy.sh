#!/bin/bash
# Uploads target/site/ (deploy/build-site.sh) to the droplet set up by deploy/droplet-setup.sh.
#
#   deploy/deploy.sh [ssh-host]        default host: graphersal-web (an alias in ~/.ssh/config,
#                                      user `deploy`)
#
# Order matters: the new playground version goes up before the index.html that names it, so a
# browser never gets a page whose files are missing. Older versions stay (an open tab still loads
# modules lazily from its own version); the newest KEEP_VERSIONS are kept, the rest removed.
# Files are compared by content (--checksum) and keep their server time when unchanged, so their
# ETag stays and browsers revalidate them with a 304.
set -euo pipefail

host=${1:-graphersal-web}
root=$(cd "$(dirname "$0")/.." && pwd)
site="$root/target/site"
web=/var/www/graphersal
KEEP_VERSIONS=5

[ -f "$site/play/VERSION" ] || { echo "deploy: run deploy/build-site.sh first" >&2; exit 1; }
version=$(cat "$site/play/VERSION")
sync=(rsync -rlzcO)   # recursive, links, compress, by checksum, no dir times (openrsync too)

# 1. the playground version (a new directory, nothing else changes yet)
"${sync[@]}" "$site/play/v/" "$host:$web/play/v/"
# 2. the entry pages that point at it
"${sync[@]}" --delete --exclude '/v/' "$site/play/" "$host:$web/play/"
# 3. the book and the schema
"${sync[@]}" --delete "$site/book/" "$host:$web/book/"
"${sync[@]}" "$site/schema/" "$host:$web/schema/"
# 4. old playground versions: keep the newest KEEP_VERSIONS (by upload time) and the current one
# shellcheck disable=SC2029  # web, KEEP_VERSIONS and version expand here, on purpose
ssh "$host" "cd $web/play/v && ls -1t | tail -n +$((KEEP_VERSIONS + 1)) | grep -vx '$version' | xargs -r rm -rf"

echo "deployed playground version $version and the book to $host"
