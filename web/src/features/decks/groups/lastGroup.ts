/** Where the deck pickers keep the group of the deck last chosen, per browser. */
const KEY = 'mtg.deckPicker.lastGroup'

/** The group of the deck last chosen in a picker, or null. */
export function lastGroup(): string | null {
  try {
    return globalThis.localStorage.getItem(KEY)
  } catch {
    return null
  }
}

/** Remembers the group of the deck just chosen; null forgets it (the deck sits in none). */
export function rememberGroup(group: string | null): void {
  try {
    if (group === null) globalThis.localStorage.removeItem(KEY)
    else globalThis.localStorage.setItem(KEY, group)
  } catch {
    // Storage may be blocked; the picker then opens the first group.
  }
}
