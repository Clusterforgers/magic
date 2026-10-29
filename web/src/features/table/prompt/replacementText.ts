/** The words of the replacement prompt: the sentence, how each effect is named and the keys. */
import type { ReplacementCandidate } from '@/protocol/generated'
import { numberKeysWords } from './choiceText'

/**
 * The one sentence above the list: the event in the engine's words (a plain sentence when it has none), then what
 * the choice means.
 */
export function replacementSentence(text: string): string {
  const event = text.trim() === '' ? 'Several replacement effects apply to one event.' : text.trim()
  return `${event} Choose which applies first; the others apply after it.`
}

/** What only a screen reader hears after the sentence. */
export function replacementKeys(count: number): string {
  return numberKeysWords(count)
}

/**
 * How each effect is named: its source card; when one card has several effects on offer, the ability or the lasting
 * effect it is; and a number for effects that still read the same. `names` holds each source's name in the order of
 * `candidates`.
 */
export function replacementLabels(
  candidates: readonly ReplacementCandidate[],
  names: readonly (string | undefined)[],
): string[] {
  const plain = candidates.map((candidate, index) => {
    const name = names[index] ?? 'A card'
    if (candidates.filter((other) => other.source === candidate.source).length < 2) return name
    return candidate.shield === undefined
      ? `${name} (ability ${candidate.ability + 1})`
      : `${name} (lasting effect)`
  })
  const total = new Map<string, number>()
  for (const label of plain) total.set(label, (total.get(label) ?? 0) + 1)
  const seen = new Map<string, number>()
  return plain.map((label) => {
    if ((total.get(label) ?? 0) < 2) return label
    const nth = (seen.get(label) ?? 0) + 1
    seen.set(label, nth)
    return `${label} ${nth}`
  })
}

/** Where an effect comes from when its card does not say: a lasting effect was made by a spell or ability; null otherwise. */
export function replacementOrigin(candidate: ReplacementCandidate): string | null {
  return candidate.shield === undefined ? null : 'A lasting effect made by a spell or ability'
}
