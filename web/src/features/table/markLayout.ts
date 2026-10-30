/** Lays the words of a grant mark into whole-word lines of a given width: measured glyph widths, no browser needed. Pure. */

/** The padding of a mark in px (`px-1` each side). */
export const MARK_PAD_PX = 8
/** What a line leaves free beside its measured width, so a font's rounding never cuts a word. */
const SLACK_PX = 1

/** The widths of 11 px semibold Geist in px by character (measured, rounded up), 7.5 for a character not listed. */
const GLYPHS: ReadonlyArray<readonly [px: number, chars: string]> = [
  [2.8, " '.,!"],
  [3.5, 'ijl:;|()I'],
  [4.8, 'ftr[]{}"-1'],
  [5.7, '/\\^'],
  [6.4, '+=<>_?7'],
  [6.5, 'acsvyz×'],
  [6.9, 'bdeghknopquxEFLTYZJ6'],
  [7.1, '9'],
  [7.3, '23458'],
  [7.6, '0KPRSX'],
  [8, 'ABCDGHUV'],
  [8.5, 'NOQ'],
  [9.1, '…%'],
  [9.9, 'mM'],
  [9.5, 'w'],
  [10.5, '@'],
  [11, 'W'],
]

const WIDTH = new Map<string, number>(
  GLYPHS.flatMap(([px, chars]) => [...chars].map((c) => [c, px] as const)),
)

/** The width of `text` in px at 11 px semibold. */
export function glyphPx(text: string): number {
  let sum = 0
  for (const c of text) sum += WIDTH.get(c) ?? 7.5
  return sum
}

/** The width of a mark that writes `text` at `fontPx`, padding included. */
export function markPx(text: string, fontPx = 11): number {
  return (glyphPx(text) * fontPx) / 11 + MARK_PAD_PX
}

/** What `layoutWords` takes. */
export interface LayoutOptions {
  /** The width a mark may use in px; unlimited when not given. */
  span?: number
  /** The text size in px. */
  fontPx?: number
  /** The most lines a mark may take. */
  maxLines?: number
}

/** Words into lines, each as many as fit; a word wider than the span stands alone. */
function pack(words: readonly string[], fits: (line: string) => boolean): string[] {
  const lines: string[] = []
  for (const word of words) {
    const last = lines.at(-1)
    if (last !== undefined && fits(`${last} ${word}`)) lines[lines.length - 1] = `${last} ${word}`
    else lines.push(word)
  }
  return lines
}

const VOWELS = 'aeiouAEIOU'

/**
 * A word that is wider than its line, in a short form that fits: its longest start that ends in a consonant, and a
 * period ("Deatht." for Deathtouch, "Intimid." for Intimidate). Null where no start fits, or for anything but a word of
 * letters ("+3/+3", "{2}"), which has no short form.
 */
export function abbreviate(word: string, fits: (text: string) => boolean): string | null {
  if (!/^[A-Za-z][A-Za-z'-]{3,}$/.test(word)) return null
  for (let kept = word.length - 1; kept >= 3; kept -= 1) {
    const last = word.charAt(kept - 1)
    if (VOWELS.includes(last) || last === '-' || last === "'") continue
    const short = `${word.slice(0, kept)}.`
    if (fits(short)) return short
  }
  return null
}

/** Whether `lines` are at most `limit` and each as narrow as the span: no line is a word that the box would cut. */
function holds(lines: readonly string[], limit: number, fits: (line: string) => boolean): boolean {
  return lines.length <= limit && lines.every(fits)
}

/** Whether the writings of every item, each in the form `pick` takes, are lines of at most `limit`, none cut. */
function tries(
  items: ReadonlyArray<{ text: string; brief: string }>,
  pick: 'text' | 'brief',
  limit: number,
  fits: (line: string) => boolean,
): string[] | null {
  const lines = pack(
    items.flatMap((item) => item[pick].split(' ')),
    fits,
  )
  return holds(lines, limit, fits) ? lines : null
}

/** The short words in up to `limit` lines, a word that is wider than the span in its abbreviation (`abbreviate`). */
function abbreviated(
  items: ReadonlyArray<{ text: string; brief: string }>,
  limit: number,
  fits: (line: string) => boolean,
): string[] | null {
  const words = items
    .flatMap((item) => item.brief.split(' '))
    .map((word) => (fits(word) ? word : (abbreviate(word, fits) ?? word)))
  const lines = pack(words, fits)
  return holds(lines, limit, fits) ? lines : null
}

/**
 * The lines of a mark for `items`: the full words in one line, else the short words in one, else the full words in
 * up to `maxLines`, else the short ones, else the short ones with a word that is wider than the mark in its
 * abbreviation ("Deatht."). Lines break at spaces only, so no word is cut by the box. When even that is too long the
 * items that fit stay, the first always, and "…" says some are left out.
 */
export function layoutWords(
  items: ReadonlyArray<{ text: string; brief: string }>,
  { span = Infinity, fontPx = 11, maxLines = 2 }: LayoutOptions = {},
): { lines: string[]; cut: boolean } {
  const fits = (line: string): boolean => markPx(line, fontPx) + SLACK_PX <= span
  const limit = Math.max(1, maxLines)
  const order: Array<['text' | 'brief', number]> = [
    ['text', 1],
    ['brief', 1],
    ['text', limit],
    ['brief', limit],
  ]
  for (const [pick, lines] of order) {
    const found = tries(items, pick, lines, fits)
    if (found !== null) return { lines: found, cut: false }
  }
  const short = abbreviated(items, limit, fits)
  if (short !== null) return { lines: short, cut: false }
  const brief = items.map((item) => item.brief.split(' '))
  const dotted = (words: readonly string[]): string[] | null => {
    const lines = pack([...words, '…'], fits)
    return lines.length <= limit ? lines : null
  }
  for (let kept = items.length - 1; kept >= 1; kept -= 1) {
    const lines = dotted(brief.slice(0, kept).flat())
    if (lines !== null) return { lines, cut: true }
  }
  // The first item alone is longer than the lines (a name, say): its last words go.
  const first = brief[0] ?? []
  for (let kept = first.length - 1; kept >= 1; kept -= 1) {
    const lines = dotted(first.slice(0, kept))
    if (lines !== null) return { lines, cut: true }
  }
  return { lines: pack([first[0] ?? '', '…'], fits).slice(0, limit), cut: true }
}
