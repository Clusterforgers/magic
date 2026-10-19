/** How a join code is shown and linked: the code is eight symbols (docs/server.md, Open games), read aloud in two fours. */

/** The code as people say and type it, two groups of four such as `K7M2-9QXA`; anything else is left as it is. */
export function codeText(code: string): string {
  return code.length === 8 ? `${code.slice(0, 4)}-${code.slice(4)}` : code
}

/** The address a friend opens to join by code on `origin` (such as `https://play.example`). */
export function joinLinkUrl(origin: string, code: string): string {
  return `${origin}/join/${encodeURIComponent(code)}`
}
