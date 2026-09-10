// Discovery for the UI audit: which pages exist, and at which widths they must work.
//
// Neither is a list anybody maintains. The pages of a built surface come from its
// filesystem and its sitemap, unioned, so an orphan page that no sitemap names is still
// audited; the pages of a surface the executable renders come from the surface's own
// anchors, crawled in ui-routes.mjs; and the widths come from the media queries the CSS
// build actually emitted, so a breakpoint added to the theme tomorrow is audited without
// this file changing.
//
// Everything here is pure: it reads a built site and returns data. The browser audit and
// the static checks both consume it, so they cannot disagree about what a page is.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, sep } from 'node:path';

import { family } from './ui-routes.mjs';

/** The narrowest viewport every page must remain usable at (WCAG 2.2 reflow, 320 CSS px). */
export const REFLOW_FLOOR = 320;

/** A width to represent the desktop end, above the largest breakpoint. */
export const DESKTOP = 1440;

/**
 * Every rendered route of a built site, from the filesystem, as paths beginning with `/`.
 * A directory holding `index.html` is a route; a stray `.html` file is one too.
 */
export function routesFromFilesystem(publicDir) {
  const routes = new Set();
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      if (!entry.name.endsWith('.html')) continue;
      const rel = relative(publicDir, full).split(sep).join('/');
      // `index.html` is the directory it sits in; at the root that directory is `/`
      if (rel === 'index.html') routes.add('/');
      else if (rel.endsWith('/index.html')) routes.add(`/${rel.slice(0, -'index.html'.length)}`);
      else routes.add(`/${rel}`);
    }
  };
  walk(publicDir);
  return routes;
}

/**
 * Every route the sitemap names, as paths. The sitemap is authoritative about what the site
 * publishes; the filesystem is authoritative about what it rendered. A page in one and not
 * the other is worth knowing about, so both are read and the union is audited.
 */
export function routesFromSitemap(publicDir) {
  const routes = new Set();
  let xml;
  try {
    xml = readFileSync(join(publicDir, 'sitemap.xml'), 'utf8');
  } catch {
    return routes;
  }
  const paths = [];
  for (const match of xml.matchAll(/<loc>([^<]+)<\/loc>/g)) {
    try {
      // A `<loc>` is an absolute URL when the site was built for an origin and a bare path
      // when it was built for a mount — `base_url = "/docs"`, which is what the build for
      // the running server uses so its assets resolve wherever it is served. Both are
      // routes; only one of them parses without a base.
      paths.push(new URL(match[1], 'http://sitemap.invalid').pathname);
    } catch {
      /* a sitemap entry that is not a location is not a route */
    }
  }
  // A project site is published under a base path (`/prismatic-majordomus/`), and the
  // sitemap's URLs carry it while the filesystem's routes do not. The prefix is the longest
  // one every entry shares, inferred rather than configured: the audit serves the built
  // directory at its own root, and comparing the two sets without stripping it would call
  // every page missing and every page an orphan.
  const prefix = commonPrefix(paths);
  for (const path of paths) {
    const stripped = prefix && path.startsWith(prefix) ? path.slice(prefix.length - 1) : path;
    routes.add(stripped.startsWith('/') ? stripped : `/${stripped}`);
  }
  return routes;
}

/** The longest leading directory every path shares, with its trailing slash, or ''. */
export function commonPrefix(paths) {
  if (paths.length === 0) return '';
  const segments = paths.map((p) => p.split('/').filter(Boolean));
  const first = segments[0] ?? [];
  let shared = 0;
  for (let i = 0; i < first.length; i += 1) {
    if (segments.every((s) => s[i] === first[i]) && segments.some((s) => s.length > i + 1)) shared += 1;
    else break;
  }
  return shared === 0 ? '' : `/${first.slice(0, shared).join('/')}/`;
}

/**
 * The audit's target set: every route either mechanism found, sorted, with where it came
 * from. A route only the sitemap knows is a rendering failure; a route only the filesystem
 * knows is an orphan. Both are reported rather than silently dropped.
 */
export function discoverPages(publicDir) {
  const fromFiles = routesFromFilesystem(publicDir);
  const fromSitemap = routesFromSitemap(publicDir);
  const all = [...new Set([...fromFiles, ...fromSitemap])].sort();
  return all.map((route) => ({
    route,
    rendered: fromFiles.has(route),
    published: fromSitemap.has(route),
  }));
}

/**
 * The breakpoints the CSS build emitted, in CSS pixels, ascending.
 *
 * Read from the compiled stylesheet rather than from a configuration file: what the build
 * produced is what the browser will apply, and a theme that redefines a breakpoint changes
 * this without anybody editing a test.
 */
export function breakpointsFromCss(cssPath) {
  const css = readFileSync(cssPath, 'utf8');
  const widths = new Set();
  for (const match of css.matchAll(/@media\s*\(min-width:\s*([0-9.]+)(px|rem)\s*\)/g)) {
    const value = Number.parseFloat(match[1]);
    if (!Number.isFinite(value)) continue;
    widths.add(Math.round(match[2] === 'rem' ? value * 16 : value));
  }
  return [...widths].sort((a, b) => a - b);
}

/**
 * The widths every page is audited at: the reflow floor, each breakpoint's boundary and the
 * pixel below it — where a layout discontinuity hides — and one desktop width. Deduplicated
 * and sorted, so a theme with two names for one width costs one visit.
 */
export function viewports(breakpoints, declared = []) {
  const widths = new Set([REFLOW_FLOOR, DESKTOP, ...declared]);
  for (const breakpoint of breakpoints) {
    widths.add(breakpoint - 1);
    widths.add(breakpoint);
  }
  return [...widths].filter((w) => w >= REFLOW_FLOOR).sort((a, b) => a - b);
}

/**
 * The widths the design declaration asks every page to be measured at, read from the
 * dataset `majordomus generate design` writes beside the stylesheet's site. They join the
 * breakpoint-derived set rather than replace it: the declaration says what a person
 * decided, the media queries say what the build did, and a page must work at both.
 * `[]` when the dataset is not there — the breakpoints still stand.
 */
export function declaredViewports(cssPath) {
  try {
    const site = dirname(dirname(cssPath));
    const data = JSON.parse(readFileSync(join(site, 'data', 'registry', 'design.json'), 'utf8'));
    return Array.isArray(data.viewports) ? data.viewports.filter((w) => Number.isInteger(w)) : [];
  } catch {
    return [];
  }
}

/**
 * How thoroughly one page is visited.
 *
 * Every page is visited at the widths where a layout most often breaks — the reflow floor,
 * one middle width and the desktop end — because a cheap invariant that skips a page is a
 * page nobody checked. The full sweep across every boundary costs a page's visit count and
 * is spent on a sample, and the sample is *derived*: one page per section of the site, the
 * first path segment, plus the root. Sections are where templates change, so this buys
 * structural coverage without anybody naming a page.
 *
 * ```
 * plan.pages.filter((p) => p.tier === 'sweep')   // the boundary sweep
 * ```
 */
export function tierPages(pages, viewportList) {
  const critical = criticalWidths(viewportList);
  const seen = new Set();
  return pages.map((page) => {
    // a page carries its own section when it was discovered under a mount: everything under
    // `/docs` shares a first segment, and tiering by that would spend the whole sweep on one
    // page of a surface with four hundred
    const section = page.section ?? page.route.split('/').filter(Boolean)[0] ?? '';
    const first = !seen.has(section);
    seen.add(section);
    return {
      ...page,
      tier: first ? 'sweep' : 'critical',
      widths: first ? viewportList : critical,
      why: first
        ? `first page of the '${section || 'root'}' section: the boundary sweep`
        : `a later page of the '${section}' section: the critical widths`,
    };
  });
}

/** The widths every page is visited at: the floor, one middle, and the desktop end. */
export function criticalWidths(viewportList) {
  const middle = viewportList[Math.floor(viewportList.length / 2)];
  return [...new Set([REFLOW_FLOOR, middle, DESKTOP])].sort((a, b) => a - b);
}

/** The whole audit target set for one directory: pages × widths, with the provenance of both. */
export function plan(publicDir, cssPath) {
  const breakpoints = breakpointsFromCss(cssPath);
  const declared = declaredViewports(cssPath);
  const widths = viewports(breakpoints, declared);
  return {
    pages: tierPages(discoverPages(publicDir), widths),
    breakpoints,
    viewports: widths,
    source: {
      pages: 'the built site: its filesystem and its sitemap, unioned',
      viewports: `the media queries of ${relative(process.cwd(), cssPath)}${declared.length ? ', and the widths share/design/tokens.yaml declares' : ''}`,
    },
  };
}

/**
 * A route as the server answers it: the surface's mount, then the page's own path.
 *
 * ```
 * mounted('/docs', '/context/')   // '/docs/context/'
 * mounted('/', '/context/')       // '/context/'
 * ```
 */
export function mounted(mount, route) {
  const prefix = mount === '/' ? '' : mount.replace(/\/+$/, '');
  return `${prefix}${route}` || '/';
}

/**
 * The audit target set over a *topology*: every surface the running executable serves,
 * each visited under the mount the topology gives it.
 *
 * This exists because a built directory does not know where it is served from. The
 * documentation is generated once and mounted at `/docs` by the executable and at the root
 * by the published site; a plan that assumed either would audit paths nobody answers. So
 * the mount is read from `majordomus web list`, which is the one place a mount is written,
 * and the audit follows the topology rather than a second opinion about it.
 *
 * A surface arrives in one of two shapes, and the difference is where its pages are, not
 * how they are held:
 *
 * - `{ id, mount, kind: 'static-directory', dir }` — a built directory, read from disk;
 * - `{ id, mount, kind: 'native-route', pages, derived }` — a surface the executable
 *   renders, whose routes ui-routes.mjs crawled out of the surface itself. `pages` is
 *   already the sample; `derived` is the whole set behind it, so the plan can say what it
 *   sampled from rather than pretend the sample is everything.
 *
 * `dir` is absolute; the routes of a native surface already carry their mount, because the
 * server is where they came from.
 */
export function planSurfaces(surfaces, cssPath) {
  const breakpoints = breakpointsFromCss(cssPath);
  const declared = declaredViewports(cssPath);
  const widths = viewports(breakpoints, declared);
  const pages = [];
  for (const surface of surfaces) {
    if (surface.kind === 'native-route') {
      for (const route of surface.pages ?? []) {
        const shape = family(route);
        pages.push({
          route,
          surface: surface.id,
          kind: 'native-route',
          family: shape,
          // a family is where a renderer changes, exactly as a directory is on a built
          // surface: one member of each takes the boundary sweep
          section: `${surface.id}:${shape}`,
          found: 'the surface, crawled from its own anchors',
        });
      }
      continue;
    }
    for (const page of discoverPages(surface.dir)) {
      pages.push({
        ...page,
        surface: surface.id,
        kind: 'static-directory',
        // the section is the page's own, inside its surface: the sweep is spent per section
        // of each surface rather than once on whichever surface sorted first
        section: `${surface.id}:${page.route.split('/').filter(Boolean)[0] ?? ''}`,
        route: mounted(surface.mount, page.route),
        found:
          page.rendered && page.published
            ? 'the built directory and its sitemap'
            : page.rendered
              ? 'the built directory alone: an orphan'
              : 'the sitemap alone: not rendered',
      });
    }
  }
  pages.sort((a, b) => (a.route < b.route ? -1 : a.route > b.route ? 1 : 0));
  return {
    pages: tierPages(pages, widths),
    breakpoints,
    viewports: widths,
    surfaces: surfaces.map((s) => surfaceReport(s)),
    source: {
      pages: describeSurfaces(surfaces),
      viewports: `the media queries of ${relative(process.cwd(), cssPath)}${declared.length ? ', and the widths share/design/tokens.yaml declares' : ''}`,
    },
  };
}

/**
 * What one surface contributed, as the report renders it.
 *
 * A native surface says how many routes it *has* beside how many the audit visits: a
 * sample reported as a total is a report that overstates its own coverage, and the
 * Cockpit's 2660 routes sampled to 108 is the whole reason the sampling is defensible.
 */
export function surfaceReport(surface) {
  const report = { id: surface.id, mount: surface.mount, kind: surface.kind ?? 'static-directory' };
  if (surface.kind !== 'native-route') return report;
  const derived = surface.derived ?? {};
  return {
    ...report,
    routes: derived.routes?.length ?? 0,
    families: Object.keys(derived.families ?? {}).length,
    sampled: surface.pages?.length ?? 0,
    fetched: derived.fetched ?? 0,
    truncated: Boolean(derived.truncated),
  };
}

/** One line saying where every page of the plan came from, per kind of surface. */
function describeSurfaces(surfaces) {
  const built = surfaces.filter((s) => s.kind !== 'native-route');
  const served = surfaces.filter((s) => s.kind === 'native-route' && (s.pages?.length ?? 0) > 0);
  const parts = [];
  if (built.length) {
    parts.push(
      `every built surface the executable serves (${built.map((s) => `${s.id} at ${s.mount}`).join(', ')}), each from its filesystem and its sitemap`,
    );
  }
  if (served.length) {
    parts.push(
      `every surface the executable renders (${served
        .map((s) => `${s.id} at ${s.mount}: ${s.pages.length} of ${s.derived?.routes?.length ?? s.pages.length} routes`)
        .join(', ')}), crawled from the surface's own anchors and sampled per family`,
    );
  }
  return parts.join('; ') || 'no surface';
}
