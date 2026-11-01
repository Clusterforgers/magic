/** The card menu's entry that marks a card as broken, to be played by hand: a fix mark made from the table. */
import type { ManualGroup } from './offers'

/** The key of the entry. */
export const FIX_MARK_KEY = 'fix-mark'

/** The group with the one entry: marks the card, or takes the mark back when it is marked already. */
export function fixMarkGroup(marked: boolean, loading = false): ManualGroup {
  return {
    id: 'fix',
    label: 'Report',
    items: [
      {
        key: FIX_MARK_KEY,
        label: marked ? 'Remove broken mark' : 'Mark as broken',
        hint: marked ? 'Marked' : 'Play by hand',
        fixMark: true,
        unavailable: loading ? 'Checking the marks' : undefined,
      },
    ],
  }
}

/** What the menu tells after a press: the mark that now stands and what it means for the card. */
export function fixMarkWords(name: string, marked: boolean): string {
  return marked
    ? `${name} is marked as broken. It is flagged to be played by hand until the card is fixed.`
    : `The broken mark on ${name} is removed.`
}
