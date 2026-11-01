import { ACTIONS } from './actions'
import { CASTING } from './casting'
import { COMBAT } from './combat'
import { PERMANENTS } from './permanents'
import type { Entry } from './types'
import { ABILITY_WORDS, SET_MECHANICS } from './words'

/** What kind of rules term a glossary entry is. */
export type GlossaryKind = 'keyword' | 'action' | 'ability word'

/** One term of the glossary: its title, every spelling that points at it and its reminder text. */
export interface Term {
  title: string
  names: readonly string[]
  text: string
  kind: GlossaryKind
}

const make = (entries: readonly Entry[], kind: GlossaryKind): Term[] =>
  entries.map(([names, text]) => {
    const list = names.split('|')
    return { title: list[0] ?? names, names: list, text, kind }
  })

/** Every term, sorted by title. */
export const TERMS: readonly Term[] = [
  ...make([...COMBAT, ...PERMANENTS, ...CASTING, ...SET_MECHANICS], 'keyword'),
  ...make(ACTIONS, 'action'),
  ...make(ABILITY_WORDS, 'ability word'),
].sort((a, b) => a.title.localeCompare(b.title))

const key = (name: string): string =>
  name
    .toLowerCase()
    .replace(/[\s-]+/g, ' ')
    .trim()

const BY_NAME = new Map<string, Term>()
for (const term of TERMS)
  for (const name of term.names) if (!BY_NAME.has(key(name))) BY_NAME.set(key(name), term)

/** The term for a keyword name as a card, the server or Scryfall spells it ("First strike", "Ward {2}" and "Ward" alike); undefined for an unknown one. */
export function termFor(name: string): Term | undefined {
  const bare = key(name)
  return BY_NAME.get(bare) ?? BY_NAME.get(bare.replace(/ ?(?:\{.*|\d+.*|x)$/, ''))
}

/** The terms whose spelling or text contains `query`, spellings first; every term for an empty query. */
export function searchTerms(query: string): readonly Term[] {
  const q = key(query)
  if (q === '') return TERMS
  const rank = (term: Term): number => {
    const names = term.names.map(key)
    if (names.includes(q)) return 0
    if (names.some((name) => name.startsWith(q))) return 1
    if (names.some((name) => name.includes(q))) return 2
    return key(term.text).includes(q) ? 3 : 4
  }
  return TERMS.filter((term) => rank(term) < 4).sort((a, b) => rank(a) - rank(b))
}

// Everyday verbs and nouns that are also terms: a card text is not marked at them, only the glossary lists them.
const PLAIN = new Set(
  [
    'Activate',
    'Attach',
    'Cast',
    'Counter',
    'Create',
    'Destroy',
    'Discard',
    'Exchange',
    'Exile',
    'Play',
    'Reveal',
    'Sacrifice',
    'Shuffle',
    'Tap',
    'Untap',
    'Double',
    'Triple',
    'Transform',
    'Mill',
    'Vote',
    'Fight',
    'Map',
    'Food',
    'Blood',
    'Treasure',
    'Enchant',
    'Equip',
    'Protection',
    'Shadow',
    'Heal',
    'Convert',
    'Draft from a spellbook',
    'Conjure',
    'Recruit',
    'Behold',
    'Prepared',
    'Harness',
    'Blight',
    'Seek',
    'Support',
    'Learn',
    'Fear',
    'Flash',
    'Reach',
    'Crew',
    'Tiered',
    'Spree',
    'Boast',
    'Channel',
    'Commander tax',
    'Commander damage',
    'Partner commander',
    'Domain',
    'Chroma',
    'Converge',
    'Opus',
    'Void',
    'Vivid',
    'Legacy',
    'Intensity',
    'Storied',
    'Descend',
    'Alliance',
    'Paradox',
    'Survival',
    'Flurry',
    'Valiant',
    'Infusion',
    'Training',
    'Increment',
    'Teamwork',
    'Renew',
    'Warp',
  ].map(key),
)

const ESCAPED = [...BY_NAME.keys()]
  .filter((name) => !PLAIN.has(name))
  .sort((a, b) => b.length - a.length)
  .map((name) => name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/ /g, '[\\s-]'))

const WORDS = new RegExp(`(?<![\\w'])(${ESCAPED.join('|')})(?![\\w'])`, 'gi')

/** A stretch of text: plain, or a keyword with its term. */
export interface Piece {
  text: string
  term?: Term
}

/** Splits `text` into plain pieces and the keyword words it names, whole words only, longest name first. */
export function pieces(text: string): Piece[] {
  const out: Piece[] = []
  let at = 0
  for (const match of text.matchAll(WORDS)) {
    const term = termFor(match[0])
    if (term === undefined) continue
    if (match.index > at) out.push({ text: text.slice(at, match.index) })
    out.push({ text: match[0], term })
    at = match.index + match[0].length
  }
  if (at < text.length) out.push({ text: text.slice(at) })
  return out
}

/** The distinct terms a text names, in the order they first appear. */
export function mentions(text: string): Term[] {
  const seen = new Set<Term>()
  for (const piece of pieces(text)) if (piece.term !== undefined) seen.add(piece.term)
  return [...seen]
}
