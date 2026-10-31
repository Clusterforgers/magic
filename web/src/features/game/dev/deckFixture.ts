/** Decks with cards for `/dev/lobby`: stored decks and precons the deck chooser lists and previews, with no server. */
import {
  cardHit,
  deckItem,
  deckView,
  legalityIssue,
  lineOf,
  reportCard,
  reportOf,
} from '@/features/decks/fakeDecks'
import type { Counts, DeckListItem, DeckView, PreconListItem, PreconView, Reason } from '@/protocol/generated'
import type { CardHit, DeckState, Support } from '@/protocol/generated'

type Colour = 'W' | 'U' | 'B' | 'R' | 'G'

/** name, type line, mana value, mana cost, colour identity */
type Pool = readonly [string, string, number, string, readonly Colour[]]

const POOL: readonly Pool[] = [
  ['Llanowar Elves', 'Creature — Elf Druid', 1, '{G}', ['G']],
  ['Birds of Paradise', 'Creature — Bird', 1, '{G}', ['G']],
  ['Eternal Witness', 'Creature — Human Shaman', 3, '{1}{G}{G}', ['G']],
  ['Acidic Slime', 'Creature — Ooze', 5, '{3}{G}{G}', ['G']],
  ['Solemn Simulacrum', 'Artifact Creature — Golem', 4, '{4}', []],
  ['Mulldrifter', 'Creature — Elemental', 5, '{4}{U}', ['U']],
  ['Courser of Kruphix', 'Enchantment Creature — Centaur', 3, '{1}{G}{G}', ['G']],
  ['Avenger of Zendikar', 'Creature — Elemental', 7, '{5}{G}{G}', ['G']],
  ['Craterhoof Behemoth', 'Creature — Beast', 8, '{5}{G}{G}{G}', ['G']],
  ['Reclamation Sage', 'Creature — Elf Shaman', 3, '{2}{G}', ['G']],
  ['Garruk Wildspeaker', 'Legendary Planeswalker — Garruk', 4, '{2}{G}{G}', ['G']],
  ['Jace, the Mind Sculptor', 'Legendary Planeswalker — Jace', 4, '{2}{U}{U}', ['U']],
  ['Counterspell', 'Instant', 2, '{U}{U}', ['U']],
  ['Swords to Plowshares', 'Instant', 1, '{W}', ['W']],
  ['Lightning Bolt', 'Instant', 1, '{R}', ['R']],
  ['Cyclonic Rift', 'Instant', 2, '{1}{U}', ['U']],
  ['Beast Within', 'Instant', 3, '{2}{G}', ['G']],
  ['Cultivate', 'Sorcery', 3, '{2}{G}', ['G']],
  ['Rampant Growth', 'Sorcery', 2, '{1}{G}', ['G']],
  ['Farseek', 'Sorcery', 2, '{1}{G}', ['G']],
  ['Wrath of God', 'Sorcery', 4, '{2}{W}{W}', ['W']],
  ['Sol Ring', 'Artifact', 1, '{1}', []],
  ['Arcane Signet', 'Artifact', 2, '{2}', []],
  ['Mind Stone', 'Artifact', 2, '{2}', []],
  ['Thought Vessel', 'Artifact', 2, '{2}', []],
  ['Rhystic Study', 'Enchantment', 3, '{2}{U}', ['U']],
  ['Smothering Tithe', 'Enchantment', 3, '{3}{W}', ['W']],
  ['Propaganda', 'Enchantment', 3, '{2}{U}', ['U']],
  ['Command Tower', 'Land', 0, '', []],
  ['Exotic Orchard', 'Land', 0, '', []],
  ['Reliquary Tower', 'Land', 0, '', []],
  ['Evolving Wilds', 'Land', 0, '', []],
]

const BASICS: readonly (readonly [string, Colour])[] = [
  ['Forest', 'G'],
  ['Island', 'U'],
  ['Plains', 'W'],
  ['Mountain', 'R'],
  ['Swamp', 'B'],
]

/** A made-up reason the game plays a card by hand. */
export const reasonOf = (text: string): Reason => ({ face: 0, span: { start: 0, end: 10 }, text })

const hit = (name: string, type: string, value: number, cost: string, identity: readonly Colour[]): CardHit =>
  cardHit(name, {
    type_line: type,
    mana_value: value,
    mana_cost: cost,
    colors: [...identity],
    color_identity: [...identity],
  })

/** A commander card: legendary, with the identity it is given. */
export const commanderHit = (name: string, identity: readonly Colour[]): CardHit =>
  hit(name, 'Legendary Creature — Fixture', 4, '{2}{G}{G}', identity)

/** What a deck of the fixture is made of. */
export interface FixtureDeck {
  id: string
  name: string
  format: string
  commanders: readonly (readonly [string, readonly Colour[]])[]
  /** Pool cards played by hand, by name. */
  hand?: readonly string[]
  /** Copies in all, the commanders included; 100 when left out. */
  size?: number
}

interface Made {
  commanders: CardHit[]
  /** Card, copies and support for the main board. */
  main: { card: CardHit; count: number; support: Support; reasons: Reason[] }[]
}

function make(deck: FixtureDeck): Made {
  const hand = new Set(deck.hand ?? [])
  const commanders = deck.commanders.map(([name, identity]) => commanderHit(name, identity))
  const size = deck.size ?? 100
  const seed = [...deck.id].reduce((sum, letter) => sum + letter.charCodeAt(0), 0)
  // A rotation of the pool, so decks differ without a random number.
  const picked = POOL.filter((_, index) => (index + seed) % 5 !== 0 || hand.size > 0)
  const main = picked.map(([name, type, value, cost, identity]) => {
    const manual = hand.has(name)
    return {
      card: hit(name, type, value, cost, identity),
      count: 1,
      support: manual ? ('manual' as const) : ('verified' as const),
      reasons: manual ? [reasonOf('The game does not resolve this ability; resolve it by hand.')] : [],
    }
  })
  const spent = main.reduce((sum, line) => sum + line.count, 0) + commanders.length
  const basics = Math.max(0, size - spent)
  BASICS.forEach(([name, colour], index) => {
    const share = Math.floor(basics / BASICS.length) + (index < basics % BASICS.length ? 1 : 0)
    if (share > 0) {
      main.push({
        card: hit(name, 'Basic Land', 0, '', [colour]),
        count: share,
        support: 'verified',
        reasons: [],
      })
    }
  })
  return { commanders, main }
}

const tally = (made: Made): Counts => {
  const all: Counts = { total: 0, verified: 0, read: 0, partial: 0, manual: 0, stale: 0, unplayable: 0 }
  for (const line of [
    ...made.main,
    ...made.commanders.map((card) => ({ card, count: 1, support: 'verified' as const })),
  ]) {
    all.total += line.count
    all[line.support] += line.count
  }
  return all
}

const stateOf = (counts: Counts): DeckState =>
  counts.unplayable > 0
    ? 'blocked'
    : counts.manual + counts.partial + counts.stale > 0
      ? 'manual_play'
      : 'verified'

/** A stored deck as the list of `GET /api/decks` shows it. */
export function fixtureListItem(deck: FixtureDeck): DeckListItem {
  const made = make(deck)
  const counts = tally(made)
  const state = stateOf(counts)
  return deckItem({
    id: deck.id,
    name: deck.name,
    format: deck.format,
    state,
    verified: state === 'verified',
    summary: { format: deck.format, state, counts, zones: [{ zone: 'main', counts }], issues: 0, to_read: 0 },
    commanders: made.commanders,
  })
}

/** A stored deck as `GET /api/decks/{id}` shows it, with the check of each card. */
export function fixtureDeckView(deck: FixtureDeck): DeckView {
  const made = make(deck)
  const state = stateOf(tally(made))
  return deckView({
    id: deck.id,
    name: deck.name,
    format: deck.format,
    state,
    verified: state === 'verified',
    cards: [
      ...made.commanders.map((card) => lineOf(card.name, 1, { zone: 'commander', card })),
      ...made.main.map((line) =>
        lineOf(line.card.name, line.count, { card: { ...line.card, support: line.support } }),
      ),
    ],
    report: reportOf(
      [
        ...made.commanders.map((card) => reportCard(card.name, { zone: 'commander' })),
        ...made.main.map((line) =>
          reportCard(line.card.name, { count: line.count, support: line.support, reasons: line.reasons }),
        ),
      ],
      state === 'blocked' ? [legalityIssue('the deck is not legal in this format')] : [],
    ),
  })
}

/** A precon as the list of `GET /api/precons` shows it. */
export function fixturePreconItem(deck: FixtureDeck & { set: string; state?: DeckState }): PreconListItem {
  const counts = tally(make(deck))
  const state = deck.state ?? stateOf(counts)
  return {
    code: deck.id,
    name: deck.name,
    set: deck.set,
    deck_type: deck.commanders.length > 0 ? 'Commander Deck' : 'Starter Deck',
    release_date: '2023-08-04',
    commanders: deck.commanders.map(([name]) => name),
    card_count: deck.size ?? 100,
    format: deck.commanders.length > 0 ? 'commander' : 'freeform',
    state,
    verified: state === 'verified',
    playable: state !== 'blocked',
    summary: { format: 'commander', state, counts, zones: [{ zone: 'main', counts }], issues: 0, to_read: 0 },
  }
}

/** A precon as `GET /api/precons/{code}` shows it. */
export function fixturePreconView(deck: FixtureDeck & { set: string; state?: DeckState }): PreconView {
  const made = make(deck)
  const list = (cards: Made['main']) =>
    cards.map((line) => ({
      name: line.card.name,
      count: line.count,
      printing_id: null,
      card: { ...line.card, support: line.support },
      support: line.support,
      reasons: line.reasons,
    }))
  return {
    precon: fixturePreconItem(deck),
    issues: [],
    boards: [
      ...(made.commanders.length === 0
        ? []
        : [
            {
              board: 'commander' as const,
              copies: made.commanders.length,
              cards: list(
                made.commanders.map((card) => ({
                  card,
                  count: 1,
                  support: 'verified' as const,
                  reasons: [],
                })),
              ),
            },
          ]),
      {
        board: 'main' as const,
        copies: made.main.reduce((sum, line) => sum + line.count, 0),
        cards: list(made.main),
      },
    ],
  }
}

const COMMANDERS: readonly (readonly [string, readonly Colour[]])[] = [
  ['Atraxa, Praetors’ Voice', ['W', 'U', 'B', 'G']],
  ['The Ur-Dragon', ['W', 'U', 'B', 'R', 'G']],
  ['Kediss, Emberclaw Familiar', ['R']],
  ['Giada, Font of Hope', ['W']],
  ['Sai, Master Thopterist', ['U']],
  ['Meren of Clan Nel Toth', ['B', 'G']],
  ['Korvold, Fae-Cursed King', ['B', 'R', 'G']],
  ['Yarok, the Desecrated', ['B', 'G', 'U']],
  ['Tatyova, Benthic Druid', ['G', 'U']],
  ['Lathril, Blade of the Elves', ['B', 'G']],
  ['Zada, Hedron Grinder', ['R']],
  ['Krenko, Mob Boss', ['R']],
]

/** The stored decks of the long list: Commander decks with commanders and a few other formats. */
export function manyStoredDecks(): FixtureDeck[] {
  return COMMANDERS.flatMap(([name, identity], index) => [
    {
      id: `many-${index}`,
      name: `${name.split(',')[0]} ${index % 2 === 0 ? 'Tribal' : 'Value'}`,
      format: 'commander',
      commanders: [[name, identity] as const],
      hand: index % 3 === 0 ? ['Beast Within', 'Rhystic Study', 'Craterhoof Behemoth'] : undefined,
    },
  ])
}

/** The precons of the long list. */
export function manyPrecons(): (FixtureDeck & { set: string })[] {
  const sets = ['c18', 'c19', 'fdc', 'frc', 'm3c', 'otc']
  return Array.from({ length: 36 }, (_, index) => {
    const [name, identity] = COMMANDERS[index % COMMANDERS.length] ?? COMMANDERS[0] ?? ['Commander', []]
    return {
      id: `${sets[index % sets.length]}-precon-${index}`,
      set: sets[index % sets.length] ?? 'c18',
      name: `${['Draconic', 'Arcane', 'Primeval', 'Planar', 'Mystic', 'Calling'][index % 6]} ${['Domination', 'Wizardry', 'Bounty', 'Rage', 'Forces'][index % 5]} ${index + 1}`,
      format: 'commander',
      commanders: [[name, identity] as const],
      hand: index % 4 === 0 ? ['Beast Within', 'Cyclonic Rift'] : undefined,
    }
  })
}
