/**
 * What a deck preview shows and how it is counted: cards grouped by type, the mana curve, the type breakdown and the
 * support counts, with adapters from a stored deck (`GET /api/decks/{id}`) and a precon (`GET /api/precons/{code}`).
 * Pure, so the numbers are tested without a page.
 */
import { COLOURS } from '@/features/builder/identity'
import { reasonLines } from '@/features/builder/report'
import type { CardHit, Counts, DeckView, DeckZone, PreconView, Reason, Support } from '@/protocol/generated'

/** The groups a deck's cards fall into on the preview, in the order they are drawn. */
export type CardKind =
  | 'creature'
  | 'planeswalker'
  | 'instant'
  | 'sorcery'
  | 'artifact'
  | 'enchantment'
  | 'battle'
  | 'land'
  | 'other'

/** The groups in drawing order. */
export const KIND_ORDER: readonly CardKind[] = [
  'creature',
  'planeswalker',
  'instant',
  'sorcery',
  'artifact',
  'enchantment',
  'battle',
  'land',
  'other',
]

/** The plural name of each group, as its heading says it. */
export const KIND_LABEL: Readonly<Record<CardKind, string>> = {
  creature: 'Creatures',
  planeswalker: 'Planeswalkers',
  instant: 'Instants',
  sorcery: 'Sorceries',
  artifact: 'Artifacts',
  enchantment: 'Enchantments',
  battle: 'Battles',
  land: 'Lands',
  other: 'Other',
}

/** One card of a deck as the preview draws it: a picture with a count, and how far the game plays it. */
export interface PreviewCard {
  oracleId: string
  name: string
  /** Copies in the deck. */
  count: number
  typeLine: string
  manaValue: number
  manaCost: string
  /** The colour identity letters of the card. */
  identity: readonly string[]
  support: Support
  /** Why the game does not run the card whole, as sentences; empty for a card that runs. */
  reasons: readonly string[]
}

/** A deck as the preview takes it: the cards by board, whatever the deck came from. */
export interface DeckPreviewData {
  name: string
  /** Where the deck comes from in words: "Your deck", "Precon FDC". */
  source: string
  /** The format key the deck was built for. */
  format: string
  /** The command zone. */
  commanders: PreviewCard[]
  /** The main deck (and a companion, which the game plays from outside the deck). */
  cards: PreviewCard[]
  sideboard: PreviewCard[]
}

/** The group of a card: its first face's types, a land before a creature before the spells (an artifact creature is a creature). */
export function kindOf(typeLine: string): CardKind {
  const front = typeLine.split(' // ')[0] ?? typeLine
  const types = front.split('—')[0] ?? front
  const has = (type: string): boolean => new RegExp(`\\b${type}\\b`).test(types)
  if (has('Land')) return 'land'
  if (has('Creature')) return 'creature'
  if (has('Planeswalker')) return 'planeswalker'
  if (has('Battle')) return 'battle'
  if (has('Instant')) return 'instant'
  if (has('Sorcery')) return 'sorcery'
  if (has('Artifact')) return 'artifact'
  if (has('Enchantment')) return 'enchantment'
  return 'other'
}

/** Copies in `cards`. */
export const copiesOf = (cards: readonly Pick<PreviewCard, 'count'>[]): number =>
  cards.reduce((sum, card) => sum + card.count, 0)

/** One group of the card grid. */
export interface CardGroup {
  kind: CardKind
  label: string
  /** Copies in the group. */
  count: number
  /** The group's cards by mana value, then name. */
  cards: PreviewCard[]
}

/** The cards in groups by type in `KIND_ORDER`, a group without cards left out. */
export function groupByKind(cards: readonly PreviewCard[]): CardGroup[] {
  return KIND_ORDER.flatMap((kind) => {
    const inKind = cards
      .filter((card) => kindOf(card.typeLine) === kind)
      .sort((a, b) => a.manaValue - b.manaValue || a.name.localeCompare(b.name))
    return inKind.length === 0
      ? []
      : [{ kind, label: KIND_LABEL[kind], count: copiesOf(inKind), cards: inKind }]
  })
}

/** The last bucket of the curve: this mana value and above. */
export const CURVE_LAST = 7

/** Spells by mana value, 0 up to `CURVE_LAST` (which holds that many and more), lands left out: copies per bucket. */
export function manaCurve(cards: readonly PreviewCard[]): number[] {
  const curve = Array.from({ length: CURVE_LAST + 1 }, () => 0)
  for (const card of cards) {
    if (kindOf(card.typeLine) === 'land') continue
    const bucket = Math.min(CURVE_LAST, Math.max(0, Math.floor(card.manaValue)))
    curve[bucket] = (curve[bucket] ?? 0) + card.count
  }
  return curve
}

/** Copies by type, in `KIND_ORDER`, a type without copies left out. */
export function typeCounts(
  cards: readonly PreviewCard[],
): { kind: CardKind; label: string; count: number }[] {
  return groupByKind(cards).map(({ kind, label, count }) => ({ kind, label, count }))
}

/** Copies by support state over `cards`. */
export function supportCounts(cards: readonly PreviewCard[]): Counts {
  const counts: Counts = { total: 0, verified: 0, partial: 0, manual: 0, stale: 0, unplayable: 0 }
  for (const card of cards) {
    counts.total += card.count
    counts[card.support] += card.count
  }
  return counts
}

/** Copies the game does not run whole (everything but Verified). */
export const byHand = (counts: Counts): number => counts.partial + counts.manual + counts.stale

/** The colour identity of the cards together in WUBRG order; empty when every card is colourless. */
export function identityOfCards(cards: readonly Pick<PreviewCard, 'identity'>[]): string[] {
  const all = new Set<string>(cards.flatMap((card) => card.identity))
  return COLOURS.filter((letter) => all.has(letter))
}

/** The commanders' identity, or the deck's own when it has no commander. */
export const identityOfDeck = (deck: DeckPreviewData): string[] =>
  identityOfCards(deck.commanders.length > 0 ? deck.commanders : [...deck.cards, ...deck.sideboard])

function previewCard(
  card: CardHit,
  count: number,
  support: Support,
  reasons: readonly Reason[],
): PreviewCard {
  return {
    oracleId: card.oracle_id,
    name: card.name,
    count,
    typeLine: card.type_line,
    manaValue: card.mana_value,
    manaCost: card.mana_cost,
    identity: card.color_identity,
    support,
    reasons: support === 'verified' ? [] : reasonLines(reasons).map((line) => line.text),
  }
}

const ZONE_BOARD: Readonly<Record<DeckZone, 'commanders' | 'cards' | 'sideboard'>> = {
  commander: 'commanders',
  main: 'cards',
  companion: 'cards',
  sideboard: 'sideboard',
}

/** A stored deck as the preview takes it: the lines with the report's support and reasons of each card. */
export function previewFromDeck(view: DeckView, source = 'Your deck'): DeckPreviewData {
  const report = new Map(view.report.cards.map((card) => [`${card.zone}:${card.oracle_id}`, card]))
  const data: DeckPreviewData = {
    name: view.name,
    source,
    format: view.format,
    commanders: [],
    cards: [],
    sideboard: [],
  }
  for (const line of view.cards) {
    const checked = report.get(`${line.zone}:${line.card.oracle_id}`)
    data[ZONE_BOARD[line.zone]].push(
      previewCard(line.card, line.count, checked?.support ?? line.card.support, checked?.reasons ?? []),
    )
  }
  return data
}

/** A precon as the preview takes it; a card the card data lacks is left out. */
export function previewFromPrecon(view: PreconView): DeckPreviewData {
  const { precon } = view
  const data: DeckPreviewData = {
    name: precon.name,
    source: [`Precon ${precon.set.toUpperCase()}`, precon.deck_type]
      .filter((part) => part !== null)
      .join(', '),
    format: precon.format,
    commanders: [],
    cards: [],
    sideboard: [],
  }
  for (const board of view.boards) {
    for (const entry of board.cards) {
      if (entry.card === null) continue
      data[ZONE_BOARD[board.board]].push(previewCard(entry.card, entry.count, entry.support, entry.reasons))
    }
  }
  return data
}
