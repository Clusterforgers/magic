/** The prompt band's layout rules (`promptBand.css`), as data the tests and the docs read. */

/** What a prompt puts in the band's middle: nothing, short choices kept beside the sentence, or choices of any size. */
export type BandChoices = 'none' | 'inline' | 'wide'

/** Below this band width (in rem) the band is a phone's: one cell per line, the answers last. */
export const BAND_PHONE_REM = 40
/** From this band width (in rem) choices of any size stand beside the sentence. */
export const BAND_WIDE_REM = 100

/** Below this band width (in rem) the answers leave out their key hints and the rest of "Cancel the spell". */
export const BAND_NARROW_REM = 64

/** What else shapes the band: a `status` beside the prompt (a note, the combat key), a prompt that lays out `cards`. */
export interface BandParts {
  status?: boolean
  cards?: boolean
}

/** The rows of the band's grid areas for a band `rem` wide holding `choices`; each row lists its cells left to right. */
export function bandAreas(
  rem: number,
  choices: BandChoices,
  { status = false, cards = false }: BandParts = {},
): string[][] {
  if (rem < BAND_PHONE_REM) {
    return [
      ['stack', 'stack'],
      ['text', 'text'],
      ['choices', 'choices'],
      ['status', 'status'],
      ['clock', 'clock'],
      ['actions', 'actions'],
    ]
  }
  const head = ['stack', 'status', 'text', 'clock', 'actions']
  if (choices === 'none') return [head]
  if (choices === 'inline' || rem >= BAND_WIDE_REM)
    return [['stack', 'status', 'text', 'choices', 'clock', 'actions']]
  if (cards) return [head, head.map(() => 'choices')]
  // The answers keep the first line to themselves and the sentence; a status starts the choices' line.
  const first = ['stack', 'text', 'clock', 'actions']
  return [first, [status ? 'status' : 'choices', 'choices', 'choices', 'choices']]
}

/** How many lines the sentence keeps before its ellipsis: one, or three beside choices of any size in a wide band. */
export function sentenceLines(rem: number, choices: BandChoices): number {
  return choices === 'wide' && rem >= BAND_WIDE_REM ? 3 : 1
}

/** Whether a box's text is cut by its line clamp: what it would take is taller or wider than what it shows. */
export function isClamped(
  box: Pick<HTMLElement, 'scrollHeight' | 'clientHeight' | 'scrollWidth' | 'clientWidth'>,
): boolean {
  return box.scrollHeight > box.clientHeight + 1 || box.scrollWidth > box.clientWidth + 1
}
