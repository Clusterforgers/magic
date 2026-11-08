/** A word diff of two wordings: the words each side has that the other lacks, marked for the bulk read table. */

/** One run of words, `changed` when the other wording lacks it. */
export interface Piece {
  text: string
  changed: boolean
}

/** Words and the spaces after them, so the pieces join back to the input. */
function tokens(text: string): string[] {
  return text.match(/\S+\s*/g) ?? []
}

const bare = (token: string): string =>
  token
    .trim()
    .toLowerCase()
    .replace(/[.,;:]+$/, '')

/** Joins adjacent tokens of the same kind into one piece. */
function pieces(words: readonly string[], kept: readonly boolean[]): Piece[] {
  const out: Piece[] = []
  words.forEach((text, i) => {
    const changed = !kept[i]
    const last = out.at(-1)
    if (last !== undefined && last.changed === changed) last.text += text
    else out.push({ text, changed })
  })
  return out
}

/** The two wordings as pieces, a word unchanged when it lies on a longest common run of words (case and closing marks aside). */
export function wordDiff(left: string, right: string): { left: Piece[]; right: Piece[] } {
  const a = tokens(left)
  const b = tokens(right)
  const lengths = Array.from({ length: a.length + 1 }, () => new Array<number>(b.length + 1).fill(0))
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      const row = lengths[i] as number[]
      const below = lengths[i + 1] as number[]
      row[j] =
        bare(a[i] as string) === bare(b[j] as string)
          ? (below[j + 1] as number) + 1
          : Math.max(below[j] as number, row[j + 1] as number)
    }
  }
  const keptA = new Array<boolean>(a.length).fill(false)
  const keptB = new Array<boolean>(b.length).fill(false)
  let i = 0
  let j = 0
  while (i < a.length && j < b.length) {
    if (bare(a[i] as string) === bare(b[j] as string)) {
      keptA[i] = true
      keptB[j] = true
      i++
      j++
    } else if ((lengths[i + 1]?.[j] ?? 0) >= (lengths[i]?.[j + 1] ?? 0)) i++
    else j++
  }
  return { left: pieces(a, keptA), right: pieces(b, keptB) }
}
