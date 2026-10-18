/**
 * The `next` search parameter of the sign-in pages: where to go once signed in. It comes from the address bar, so
 * anyone can write anything in it; only a plain path on this site gets through, and never a fragment (a seat token
 * must not travel in a query string, which the server sees).
 */

// A stand-in origin to resolve against: whatever resolves to another origin is not a path on this site.
const SITE = 'https://app.invalid'
/** The longest `next` accepted, in characters. */
export const MAX_NEXT_CHARS = 1024
// Pages that would send a signed-in visitor straight back to the sign-in form.
const AUTH_PAGES: ReadonlySet<string> = new Set(['/login', '/register'])

/**
 * `value` as a path to navigate to after signing in, or null when it is not one. Accepts only a string that starts
 * with a single `/` and resolves, the way a browser reads it, to the same site: no `//host` (protocol-relative), no
 * backslash (read as a slash), no control character (a tab or newline is deleted by the address parser, which can
 * turn `/<tab>/host` into `//host`), no scheme. The result is the normalised path and query, without the fragment,
 * and never the sign-in pages themselves.
 */
export function sanitizeNext(value: unknown): string | null {
  if (typeof value !== 'string' || value === '' || value.length > MAX_NEXT_CHARS) return null
  if (!value.startsWith('/') || value.startsWith('//') || value.includes('\\') || /\p{Cc}/u.test(value)) {
    return null
  }
  let url: URL
  try {
    url = new URL(value, SITE)
  } catch {
    return null
  }
  if (url.origin !== SITE) return null
  const path = `${url.pathname}${url.search}`
  // Dot segments can leave `//host` at the front of the path (`/.//host`), which a router may take for another site.
  if (path.startsWith('//')) return null
  if (AUTH_PAGES.has(url.pathname.toLowerCase().replace(/\/+$/, ''))) return null
  return path
}

/**
 * The search of the sign-in pages: `next` only when it is safe. The key is always there, `undefined` when the value
 * was refused: the router merges what a page validates over the raw search of the address, so leaving the key out
 * would let the unchecked value through.
 */
export function parseAuthSearch(search: Record<string, unknown>): { next?: string | undefined } {
  return { next: sanitizeNext(search.next) ?? undefined }
}
