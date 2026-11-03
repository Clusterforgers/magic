import { mentions, termFor } from '@/lib/glossary'
import type { Term } from '@/lib/glossary'
import { abilityText, keywordNames } from './cardModel'
import type { ObjectView } from '@/protocol/generated'

/** The glossary terms of a card: its keywords first, then every keyword word its ability texts mention, each once. */
export function reminderTerms(object: ObjectView, extra: readonly string[] = []): Term[] {
  const seen = new Set<Term>()
  for (const name of keywordNames(object)) {
    const term = termFor(name)
    if (term !== undefined) seen.add(term)
  }
  for (const text of [...object.abilities.map(abilityText), ...extra])
    for (const term of mentions(text)) seen.add(term)
  return [...seen]
}
