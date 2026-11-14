/**
 * Dev-only visual directions (docs/ui-audit.md, Directions): `?look=a|b|c` on any page of the dev server sets
 * `data-look` on `<html>`, and `looks.css` restyles the tokens and the table under it. `main.tsx` loads this module only
 * when `import.meta.env.DEV`, so a production build carries none of it.
 */

/** Text that appears in the dev bundle only; `devBuild.test.ts` holds it out of the production build. */
export const LOOKS_MARKER = 'dev-looks-switch'

/** The directions: `a` Spellforge (dark fantasy table), `b` Clean Slate (flat, card first), `c` Nightglass (glass and seat colors). */
export const LOOKS = ['a', 'b', 'c'] as const

/** One of `LOOKS`. */
export type Look = (typeof LOOKS)[number]

const KEY = 'mtg.dev.look'

/** Reads `look` from an address's query; `off` clears a remembered look, anything else unknown is no answer. */
export function lookOf(search: string): Look | 'off' | undefined {
  const value = new URLSearchParams(search).get('look')
  if (value === 'off') return 'off'
  return LOOKS.find((look) => look === value)
}

/** The look to wear: the address's, else the one this tab remembered. */
export function chooseLook(search: string, remembered: string | null): Look | undefined {
  const asked = lookOf(search)
  if (asked === 'off') return undefined
  return asked ?? LOOKS.find((look) => look === remembered)
}

function remembered(): string | null {
  try {
    return sessionStorage.getItem(KEY)
  } catch {
    return null
  }
}

function remember(look: Look | undefined): void {
  try {
    if (look === undefined) sessionStorage.removeItem(KEY)
    else sessionStorage.setItem(KEY, look)
  } catch {
    // A tab without storage forgets the look on the next page; the address still sets it.
  }
}

/** Puts the chosen look on `<html>` and keeps it for the tab, so links between dev pages keep it. */
export function installLook(): void {
  const look = chooseLook(window.location.search, remembered())
  remember(look)
  const root = document.documentElement
  if (look === undefined) {
    delete root.dataset.look
    return
  }
  root.dataset.look = look
  root.dataset.lookMarker = LOOKS_MARKER
  void import('./looks.css')
}
