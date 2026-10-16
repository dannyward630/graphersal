// The version shown in the header: the engine version (`info().version`) and, for the WebAssembly
// playground, the commit its module was built from (`pkg/build-info.json`, written by
// playground/build.sh). The dev server has no build file: it shows the engine version only.

const BUILD_INFO_URL = new URL('../pkg/build-info.json', import.meta.url);

/**
 * The build file, or null when there is none (an older build, the dev server) or it is unreadable.
 * @returns {Promise<{commit: string, describe: string}|null>}
 */
export async function loadBuildInfo() {
  try {
    const r = await fetch(BUILD_INFO_URL, { cache: 'no-store' });
    if (!r.ok) return null;
    const data = await r.json();
    return data && typeof data.commit === 'string' ? data : null;
  } catch {
    return null;
  }
}

/**
 * The header's text and tooltip. A build exactly at the release tag `v<version>` shows the
 * version alone; any other build adds its `git describe` (commits after the tag, the commit,
 * `-dirty` for uncommitted changes).
 * @param {string|null|undefined} version  the engine version, e.g. "0.2.0"
 * @param {{commit?: string, describe?: string}|null|undefined} build
 * @returns {{text: string, title: string}|null}  null without a version
 */
export function versionLabel(version, build) {
  if (!version) return null;
  const tag = `v${version}`;
  const describe = build && build.describe ? build.describe : '';
  const commit = build && build.commit ? build.commit : '';
  const text = !describe || describe === tag ? tag : `${tag} · ${describe}`;
  const title = `Engine ${version}` + (commit ? `, built from commit ${commit}` + (describe.endsWith('-dirty') ? ' with uncommitted changes' : '') : '');
  return { text, title };
}
