/** The prompt band's layout rules (`promptBand.css`), as data the tests and the docs read. */

/**
 * What a prompt puts in the band besides the sentence and the answers: nothing, short choices kept beside the sentence
 * (`inline`), or choices with no column or sheet to hold them (`fallback`: the opening hand on a phone), which take a
 * line of their own and grow the sheet. Everything else a prompt chooses among is drawn in the Stack column or the
 * decision sheet and is no cell of the band.
 */
export type BandChoices = 'none' | 'inline' | 'fallback'

/** Below this band width (in rem) the band is a phone's: two lines, the answers on the second. */
export const BAND_PHONE_REM = 40

/** Below this band width (in rem) the stack's label goes to a reader and "+2 more" reads "+2". */
export const BAND_NARROW_REM = 64

/** Below this band width (in rem) the answers leave out their key hints and the rest of "Cancel the spell". */
export const BAND_HINTS_REM = 56

/** The lines the sentence keeps before its ellipsis. */
export const SENTENCE_LINES = 2

/** What else shapes the band. */
export interface BandParts {
  /** The sentence is read aloud and not drawn (a payment's): the meter stands in its place. */
  quiet?: boolean
}

/** The rows of the band's grid areas for a band `rem` wide holding `choices`; each row lists its cells left to right. */
export function bandAreas(rem: number, choices: BandChoices, { quiet = false }: BandParts = {}): string[][] {
  if (rem < BAND_PHONE_REM) {
    // The second line is one cell: the meter, a status or short choices from its left, the answers from its right.
    const foot = ['foot', 'foot', 'foot']
    return choices === 'fallback'
      ? [['lead', 'text', 'clock'], ['choices', 'choices', 'choices'], foot]
      : [['lead', 'text', 'clock'], foot]
  }
  if (choices === 'fallback') {
    return [
      ['lead', 'status', 'text', 'meter', 'meter', 'clock', 'actions'],
      Array<string>(7).fill('choices'),
    ]
  }
  if (quiet) return [['lead', 'status', 'meter', 'choices', 'clock', 'actions']]
  return [['lead', 'status', 'text', 'meter', 'choices', 'clock', 'actions']]
}

/** Whether a box's text is cut by its line clamp: what it would take is taller or wider than what it shows. */
export function isClamped(
  box: Pick<HTMLElement, 'scrollHeight' | 'clientHeight' | 'scrollWidth' | 'clientWidth'>,
): boolean {
  return box.scrollHeight > box.clientHeight + 1 || box.scrollWidth > box.clientWidth + 1
}
