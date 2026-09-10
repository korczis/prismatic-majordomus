// The routes of a surface the executable renders itself.
//
// A static surface is a directory: its pages are files on disk and a sitemap beside them,
// and ui-discover.mjs reads both. A surface the executable answers at request time has no
// directory to read, and until this module existed the audit skipped every one of them —
// the whole Cockpit, the server's home page and the Swagger shell were outside the standard
// the rest of the repository is held to. ADR 0022 says the page set is discovered rather
// than listed; ADR 0036 named this as the gap that keeps that guarantee from reaching every
// surface. This is the missing half of the discovery, and it is the only implementation of
// it: the Cockpit probe imports it too, so the two browser instruments cannot disagree
// about what a Cockpit route is.
//
// **What replaces the directory is the surface itself.** A page a reader can reach is a
// page something links to, so the route set of a served surface is the transitive closure
// of its own anchors, inside its own mount. Nothing here names a route: the mounts come
// from `majordomus web list`, the entry is the mount, and every route after that was
// written by the surface. A page added to the Cockpit tomorrow is audited tomorrow, and a
// page linked from nowhere is out of reach of this crawl for exactly the reason it is out
// of reach of a reader.
//
// **The crawl is bounded by what it learns, not by a depth.** A route is expanded while
// routes of its shape are still yielding routes nobody had seen; a paginated listing is
// therefore followed to its last page, and a thousand leaf pages whose only links go back
// to the shell cost two fetches between them. That is what makes a surface with 2400
// routes discoverable in about a hundred requests.
//
// Everything here is pure with respect to the network: the caller passes the fetcher, so
// the discipline can be tested against a fixture with no server and no browser.

/** How many routes of one shape may yield nothing new before that shape stops expanding. */
export const PATIENCE = 2;

/** A ceiling on the crawl, so a surface that links in a cycle cannot run forever. */
export const FETCH_BUDGET = 600;

/** How many members of one family the audit visits. See [`sample`]. */
export const FAMILY_SAMPLE = 4;

/** How many pages the crawl fetches at once. A fetch is waiting; a batch keeps the order. */
export const CRAWL_JOBS = 4;

/**
 * The anchors of a document, as the paths they point at.
 *
 * Anchors, not links: `<a href>` is what a reader can follow, while `<link href>` is a
 * stylesheet and an icon. Reading only anchors is why nothing here needs a list of file
 * extensions to tell a page from an asset — the markup already made that distinction.
 *
 * ```
 * anchors('<a class="x" href="/cockpit/health">h</a>')   // ['/cockpit/health']
 * ```
 */
export function anchors(html, base = 'http://surface.invalid') {
  const out = [];
  for (const match of html.matchAll(/<a\b[^>]*?\shref="([^"]*)"/gi)) {
    const href = match[1].replace(/&amp;/g, '&').trim();
    if (!href || href.startsWith('#')) continue;
    let url;
    try {
      url = new URL(href, base);
    } catch {
      continue; // a `mailto:` or a template that did not render is not a route
    }
    if (url.origin !== new URL(base).origin) continue; // another host is not this surface
    out.push(`${url.pathname}${url.search}`);
  }
  return out;
}

/**
 * The mount that owns a route: the longest declared mount it lies under, or `undefined`.
 *
 * The topology is what settles this. The home page is mounted at `/` and every other
 * surface under it, so "the routes of the home surface" is not "everything the server
 * answers" — it is everything no narrower mount has claimed.
 *
 * ```
 * owner('/cockpit/health', ['/', '/cockpit'])   // '/cockpit'
 * owner('/about', ['/', '/cockpit'])            // '/'
 * ```
 */
export function owner(route, mounts) {
  const path = route.split('?')[0];
  let best;
  for (const mount of mounts) {
    const normal = mount === '/' ? '/' : mount.replace(/\/+$/, '');
    const under = normal === '/' ? true : path === normal || path.startsWith(`${normal}/`);
    if (!under) continue;
    if (best === undefined || normal.length > best.length) best = normal;
  }
  return best;
}

/**
 * The shape a route shares with its siblings: what the audit samples rather than visits.
 *
 * A family is one renderer over many records. Two signals say so and both are in the route
 * itself: a route with a query varies in its *arguments*, so the path stays and the values
 * go; a route without one varies in its *last segment*, so the parent stays and the segment
 * goes. Nothing about the Cockpit is encoded here — the same rule files a documentation
 * page and a Swagger operation.
 *
 * ```
 * family('/cockpit/capabilities/plan.next')   // '/cockpit/capabilities/*'
 * family('/cockpit/capabilities?module=adr')  // '/cockpit/capabilities?module'
 * family('/cockpit')                          // '/*'
 * ```
 */
export function family(route) {
  const [path, query = ''] = route.split('?');
  if (query) {
    const keys = [...new Set([...new URLSearchParams(query).keys()])].sort().join('&');
    return `${path.replace(/\/+$/, '') || '/'}?${keys}`;
  }
  const segments = path.split('/').filter(Boolean);
  if (segments.length === 0) return '/';
  return `/${[...segments.slice(0, -1), '*'].join('/')}`;
}

/** `n` members of a list, evenly spread, keeping the first and the last. */
export function spread(list, n) {
  if (list.length <= n) return [...list];
  if (n <= 1) return [list[0]];
  const step = (list.length - 1) / (n - 1);
  return [...new Set(Array.from({ length: n }, (_, i) => list[Math.round(i * step)]))];
}

/**
 * Every route of one served surface, crawled from its mount.
 *
 * `fetchText(route)` returns the surface's markup for a route, or `null` when the route is
 * not a document — a JSON answer, a redirect, an error. `mounts` is every mount the
 * topology declares, so a link into another surface is that surface's business.
 *
 * Returns the entry, the routes the entry advertises (`navigation` — what the surface
 * itself puts in front of a reader), every route the crawl reached, those routes grouped
 * into families, and what the crawl cost. `truncated` is true when the budget stopped it,
 * which the report says out loud rather than passing off as a complete set.
 */
export async function crawl({
  mount,
  mounts = [mount],
  fetchText,
  patience = PATIENCE,
  budget = FETCH_BUDGET,
  jobs = CRAWL_JOBS,
}) {
  const entry = mount === '/' ? '/' : mount.replace(/\/+$/, '');
  const mine = (route) => owner(route, mounts) === entry;

  const seen = new Set([entry]);
  const queue = [entry];
  const barren = new Map();
  let navigation = [];
  let fetched = 0;
  let truncated = false;
  // whether the mount answered a document at all. A surface whose entry is JSON, an event
  // stream or a protocol endpoint has no pages, which is a different claim from "the crawl
  // found none" and is reported as itself.
  let document = false;

  while (queue.length) {
    // one batch, taken in discovery order and accounted after it: a fetch is waiting, and
    // taking a fixed slice keeps the crawl's decisions the same however fast each answered
    const batch = [];
    while (batch.length < jobs && queue.length) {
      const route = queue.shift();
      if ((barren.get(family(route)) ?? 0) >= patience) continue;
      batch.push(route);
    }
    if (batch.length === 0) continue;
    if (fetched + batch.length > budget) {
      truncated = true;
      break;
    }
    const bodies = await Promise.all(batch.map((route) => fetchText(route).catch(() => null)));
    fetched += batch.length;
    for (let i = 0; i < batch.length; i += 1) {
      const route = batch[i];
      const body = bodies[i];
      if (body === null || body === undefined) continue;
      const found = [...new Set(anchors(body))].filter(mine);
      if (route === entry) {
        document = true;
        navigation = found.filter((r) => r !== entry);
      }
      let fresh = 0;
      for (const link of found) {
        if (seen.has(link)) continue;
        seen.add(link);
        queue.push(link);
        fresh += 1;
      }
      const shape = family(route);
      barren.set(shape, fresh === 0 ? (barren.get(shape) ?? 0) + 1 : 0);
    }
  }

  const routes = document ? [...seen].sort() : [];
  const families = {};
  for (const route of routes) (families[family(route)] ??= []).push(route);
  return {
    entry,
    document,
    navigation: [...new Set(navigation)].sort(),
    routes,
    families,
    fetched,
    truncated,
  };
}

/**
 * The routes the audit visits on a served surface: everything the surface advertises, and
 * `n` members of every family it generates.
 *
 * A family shares one renderer, so what varies across its members is the record behind
 * them; a handful spread across the sorted set exercises the shortest identity and the
 * longest, the emptiest record and the fullest, for the cost of a handful. `n` is a budget
 * rather than a list — it is spent on whatever members exist, and a family with fewer than
 * `n` members is visited whole, which is why no navigation entry is ever dropped.
 */
export function sample(derived, n = FAMILY_SAMPLE) {
  if (!derived.document) return [];
  const chosen = new Set([derived.entry, ...derived.navigation]);
  for (const routes of Object.values(derived.families)) {
    for (const route of spread(routes, n)) chosen.add(route);
  }
  return [...chosen].sort();
}

/**
 * A fetcher over a running origin that answers with markup or with `null`.
 *
 * A surface is a *page* surface when its mount answers a document to something that asked
 * for one. That is the derived discriminator between the Cockpit and `/api/v1`: nothing
 * here decides which surfaces have pages, the server does, by what it sends back to an
 * `Accept: text/html` request. A JSON API, an event stream and an MCP endpoint all fail it,
 * and none of them had to be named.
 */
export function documentFetcher(origin, fetchImpl = fetch) {
  return async (route) => {
    const response = await fetchImpl(`${origin}${route}`, { headers: { accept: 'text/html' } });
    if (!response.ok) return null;
    if (!(response.headers.get('content-type') ?? '').includes('text/html')) return null;
    return response.text();
  };
}

/**
 * Every served surface of a topology, with the routes each one has.
 *
 * `topology` is what `majordomus web list --format json` answers. A static surface keeps
 * its directory, because a directory is a better answer than a crawl: it holds the orphan
 * pages a crawl would never reach. A surface the executable renders is crawled, and one
 * whose mount is not a document — the JSON API, the event stream, MCP — is reported as
 * having no pages rather than quietly dropped, because "this surface has no pages" and
 * "nobody looked" are different claims.
 */
export async function servedSurfaces(topology, { origin, root, fetchText, isDirectory }) {
  const surfaces = Array.isArray(topology) ? topology : topology.surfaces;
  const mounts = surfaces.map((s) => (s.mount === '/' ? '/' : s.mount.replace(/\/+$/, '')));
  const fetcher = fetchText ?? documentFetcher(origin);
  const out = [];
  for (const surface of surfaces) {
    if (surface.availability === 'published-only') continue; // not served: its publication audits its own tree
    if (surface.kind === 'static-directory') {
      if (!surface.artifact) continue;
      const dir = `${root}/${surface.artifact}`;
      if (isDirectory && !isDirectory(dir)) continue; // a surface whose producer has not run is not a target
      out.push({ id: surface.id, mount: surface.mount, kind: surface.kind, dir });
      continue;
    }
    const derived = await crawl({ mount: surface.mount, mounts, fetchText: fetcher });
    out.push({
      id: surface.id,
      mount: surface.mount,
      kind: surface.kind,
      derived,
      pages: sample(derived),
    });
  }
  return out;
}
