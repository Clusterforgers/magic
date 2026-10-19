/** The deck behind `/dev/builder`: a card of each support state and illegal lines, as a server would answer it. */
import {
  cardHit,
  deckView,
  legalityIssue,
  oracleIdOf,
  reportCard,
  reportOf,
} from '@/features/decks/fakeDecks'
import { formatLabel } from '@/features/decks/format'
import type {
  CardDetail,
  CardHit,
  CheckRequest,
  DeckLineView,
  DeckState,
  DeckView,
  DeckZone,
  Issue,
  Reason,
  ReportCard,
  Support,
} from '@/protocol/generated'
import type { CheckScript } from '../fakeCheck'

/** The text the dev page prints, so a production bundle can be searched for any trace of the fixture. */
export const BUILDER_FIXTURE_MARKER = 'dev-builder-fixture-3c9e'

/** The decks `/dev/builder?deck=` can show. */
export const DEV_DECKS = ['mixed', 'manual', 'verified', 'changed', 'unchecked'] as const

/** One of the decks `/dev/builder` shows. */
export type DevDeck = (typeof DEV_DECKS)[number]

/** One face of a card as the preview shows it. */
interface FaceText {
  name: string
  cost: string
  type: string
  text: string
  /** Power and toughness. */
  stats?: [string, string]
}

/** A clause the game cannot run: the words of a face's text that it covers (the whole text when left out) and why. */
interface Clause {
  face?: number
  says?: string
  why: string
}

interface Entry {
  name: string
  zone?: DeckZone
  count: number
  support: Support
  /** The type line and cost of the card as a search hit shows them (the faces joined with `//`). */
  type?: string
  cost?: string
  faces?: FaceText[]
  clauses?: Clause[]
}

const WHOLE = 'no rules for this card yet; the table resolves the whole text'

// The entries by support state: the first of each state is the one the others of the fixture are built around.
const CLEAN: Entry[] = [
  {
    name: 'Krenko, Mob Boss',
    zone: 'commander',
    count: 1,
    support: 'verified',
    type: 'Legendary Creature — Goblin Warrior',
    cost: '{2}{R}{R}',
    faces: [
      {
        name: 'Krenko, Mob Boss',
        cost: '{2}{R}{R}',
        type: 'Legendary Creature — Goblin Warrior',
        text: '{T}: Create X 1/1 red Goblin creature tokens, where X is the number of Goblins you control.',
        stats: ['3', '3'],
      },
    ],
  },
  {
    name: 'Goblin Guide',
    count: 1,
    support: 'verified',
    type: 'Creature — Goblin Scout',
    cost: '{R}',
    faces: [
      {
        name: 'Goblin Guide',
        cost: '{R}',
        type: 'Creature — Goblin Scout',
        text: "Haste\nWhenever Goblin Guide attacks, defending player reveals the top card of their library. If it's a land card, that player puts it into their hand.",
        stats: ['2', '2'],
      },
    ],
  },
  {
    name: 'Duress',
    zone: 'sideboard',
    count: 2,
    support: 'verified',
    type: 'Sorcery',
    cost: '{B}',
    faces: [
      {
        name: 'Duress',
        cost: '{B}',
        type: 'Sorcery',
        text: 'Target opponent reveals their hand. You choose a noncreature, nonland card from it. That player discards that card.',
      },
    ],
  },
]
const BY_HAND: Entry[] = [
  {
    name: 'Goblin Rabblemaster',
    count: 1,
    support: 'partial',
    type: 'Creature — Goblin Warrior',
    cost: '{2}{R}',
    faces: [
      {
        name: 'Goblin Rabblemaster',
        cost: '{2}{R}',
        type: 'Creature — Goblin Warrior',
        text: 'Other Goblin creatures you control attack each combat if able.\nAt the beginning of combat on your turn, create a 1/1 red Goblin creature token with haste.\nWhenever Goblin Rabblemaster attacks, it gets +1/+0 until end of turn for each other attacking Goblin.',
        stats: ['2', '2'],
      },
    ],
    clauses: [
      {
        says: 'Other Goblin creatures you control attack each combat if able.',
        why: 'the engine does not run attack requirements yet',
      },
      {
        says: 'for each other attacking Goblin',
        why: 'the engine cannot count the other attackers yet',
      },
    ],
  },
  {
    name: 'Bonecrusher Giant // Stomp',
    count: 1,
    support: 'partial',
    type: 'Creature — Giant // Instant — Adventure',
    cost: '{2}{R} // {1}{R}',
    faces: [
      {
        name: 'Bonecrusher Giant',
        cost: '{2}{R}',
        type: 'Creature — Giant',
        text: "Whenever Bonecrusher Giant becomes the target of a spell, Bonecrusher Giant deals 2 damage to that spell's controller.",
        stats: ['4', '3'],
      },
      {
        name: 'Stomp',
        cost: '{1}{R}',
        type: 'Instant — Adventure',
        text: "Damage can't be prevented this turn. Stomp deals 2 damage to any target.\nThen exile this card. You may cast the creature later from exile.",
      },
    ],
    clauses: [
      {
        face: 1,
        says: "Damage can't be prevented this turn.",
        why: 'the engine does not run damage prevention yet',
      },
    ],
  },
  {
    name: 'Chaos Orb',
    count: 1,
    support: 'manual',
    type: 'Artifact',
    cost: '{2}',
    faces: [
      {
        name: 'Chaos Orb',
        cost: '{2}',
        type: 'Artifact',
        text: '{1}, {T}: If Chaos Orb is on the battlefield, flip Chaos Orb onto the battlefield from a height of at least one foot. If Chaos Orb turns over completely at least once during the process, destroy all nontoken permanents it touches. Then destroy Chaos Orb.',
      },
    ],
    clauses: [{ why: WHOLE }],
  },
  {
    name: 'Goblin Welder',
    count: 1,
    support: 'stale',
    type: 'Creature — Goblin Artificer',
    cost: '{R}',
    faces: [
      {
        name: 'Goblin Welder',
        cost: '{R}',
        type: 'Creature — Goblin Artificer',
        text: "{T}: Choose target artifact a player controls and target artifact card in that player's graveyard. If both targets are still legal as this ability resolves, that player simultaneously sacrifices the artifact and returns the artifact card to the battlefield.",
        stats: ['1', '1'],
      },
    ],
    clauses: [{ why: 'the oracle text changed since the rules were written' }],
  },
  {
    name: 'Pithing Needle',
    zone: 'sideboard',
    count: 1,
    support: 'manual',
    type: 'Artifact',
    cost: '{1}',
    faces: [
      {
        name: 'Pithing Needle',
        cost: '{1}',
        type: 'Artifact',
        text: "As Pithing Needle enters the battlefield, choose a card name.\nActivated abilities of sources with the chosen name can't be activated unless they're mana abilities.",
      },
    ],
    clauses: [{ why: WHOLE }],
  },
]
const BLOCKING: Entry[] = [
  {
    name: 'Cogwork Librarian',
    count: 1,
    support: 'unplayable',
    type: 'Artifact Creature — Construct',
    cost: '{3}',
    faces: [
      {
        name: 'Cogwork Librarian',
        cost: '{3}',
        type: 'Artifact Creature — Construct',
        text: 'Draft Cogwork Librarian face up.\nAs you draft a card, you may remove Cogwork Librarian from your draft pool from the game. If you do, put the drafted card back into the booster pack.',
        stats: ['2', '1'],
      },
    ],
    clauses: [
      {
        says: 'Draft Cogwork Librarian face up.',
        why: 'the card changes how a draft works, which this game does not run',
      },
    ],
  },
]
const ILLEGAL: Entry[] = [
  {
    name: 'Lightning Bolt',
    count: 4,
    support: 'verified',
    type: 'Instant',
    cost: '{R}',
    faces: [
      {
        name: 'Lightning Bolt',
        cost: '{R}',
        type: 'Instant',
        text: 'Lightning Bolt deals 3 damage to any target.',
      },
    ],
  },
  {
    name: 'Black Lotus',
    count: 1,
    support: 'verified',
    type: 'Artifact',
    cost: '{0}',
    faces: [
      {
        name: 'Black Lotus',
        cost: '{0}',
        type: 'Artifact',
        text: '{T}, Sacrifice Black Lotus: Add three mana of any one color.',
      },
    ],
  },
]

// The longest card name there is (141 characters), which a search row has to fit at any width.
const LONGEST =
  'Our Market Research Shows That Players Like Really Long Card Names So We Made this Card to Have the Absolute Longest Card Name Ever Elemental'

const SEARCH_ONLY: Entry[] = [
  {
    name: 'Mountain',
    count: 0,
    support: 'verified',
    faces: [{ name: 'Mountain', cost: '', type: 'Basic Land — Mountain', text: '({T}: Add {R}.)' }],
  },
  {
    name: LONGEST,
    count: 0,
    support: 'manual',
    type: 'Creature — Elemental',
    cost: '{1}{G}',
    faces: [
      { name: LONGEST, cost: '{1}{G}', type: 'Creature — Elemental', text: 'Trample', stats: ['2', '2'] },
    ],
    clauses: [{ why: WHOLE }],
  },
  {
    name: 'Delver of Secrets // Insectile Aberration',
    count: 0,
    support: 'verified',
    type: 'Creature — Human Wizard // Creature — Human Insect',
    cost: '{U}',
    faces: [
      {
        name: 'Delver of Secrets',
        cost: '{U}',
        type: 'Creature — Human Wizard',
        text: 'At the beginning of your upkeep, look at the top card of your library. You may reveal that card. If an instant or sorcery card is revealed this way, transform Delver of Secrets.',
        stats: ['1', '1'],
      },
      {
        name: 'Insectile Aberration',
        cost: '',
        type: 'Creature — Human Insect',
        text: 'Flying',
        stats: ['3', '2'],
      },
    ],
  },
]

const ALL: Entry[] = [...CLEAN, ...BY_HAND, ...BLOCKING, ...ILLEGAL, ...SEARCH_ONLY]

const hitOf = (entry: Entry): CardHit =>
  cardHit(entry.name, {
    support: entry.support,
    type_line: entry.type ?? 'Basic Land — Mountain',
    mana_cost: entry.cost ?? '',
  })

// A position counts Unicode scalar values, as the server's spans do.
const positions = (text: string): number => Array.from(text).length

/** The clauses of a card as the server reports them, each over a span of its face's text. */
function reasonsOf(entry: Entry): Reason[] {
  return (entry.clauses ?? []).map((clause) => {
    const face = clause.face ?? 0
    const text = entry.faces?.[face]?.text ?? ''
    if (clause.says === undefined) return { face, span: { start: 0, end: positions(text) }, text: clause.why }
    const at = text.indexOf(clause.says)
    if (at < 0) throw new Error(`the fixture text of ${entry.name} does not say "${clause.says}"`)
    const start = positions(text.slice(0, at))
    return { face, span: { start, end: start + positions(clause.says) }, text: clause.why }
  })
}

/** The cards of the fixture as a search shows them, for the card server behind the page. */
export const fixtureCards = (): CardHit[] => ALL.map(hitOf)

/** The details of the fixture cards as `GET /api/cards/{oracle_id}` shows them, with their clauses over the faces' text. */
export function fixtureDetails(): CardDetail[] {
  return ALL.map((entry) => {
    const card = hitOf(entry)
    const faces = (entry.faces ?? []).map((face) => ({
      name: face.name,
      mana_cost: face.cost,
      type_line: face.type,
      oracle_text: face.text,
      power: face.stats?.[0] ?? null,
      toughness: face.stats?.[1] ?? null,
      loyalty: null,
      defense: null,
      colors: card.colors,
    }))
    return {
      card,
      oracle_text: faces.map((face) => face.oracle_text).join('\n//\n'),
      keywords: [],
      faces,
      legalities: { commander: 'legal' },
      printings: [],
      rulings: [],
      reasons: reasonsOf(entry),
    }
  })
}

// Singleton formats allow one copy of a card but a basic land; the others four.
const isBasic = (name: string): boolean => name === 'Mountain'
const BANNED: readonly string[] = ['Black Lotus']

/** The ways a list breaks the rules of its format that the fixture knows: its size, copies and a banned card. */
export function fixtureLegality(request: CheckRequest): Issue[] {
  const format = request.format ?? 'commander'
  const lines = request.cards ?? []
  const names = new Map(ALL.map((entry) => [oracleIdOf(entry.name), entry.name]))
  const nameOf = (oracleId: string): string => names.get(oracleId) ?? oracleId
  const size = lines.filter((line) => line.zone !== 'sideboard').reduce((sum, line) => sum + line.count, 0)
  const issues: Issue[] = []
  if (format === 'commander' && size !== 100) {
    issues.push(
      legalityIssue(`a Commander deck has exactly 100 cards with its commanders, this one has ${size}`, {
        code: 'deck_size',
      }),
    )
  } else if (format !== 'commander' && size < 60) {
    issues.push(
      legalityIssue(`a ${formatLabel(format)} deck has at least 60 cards, this one has ${size}`, {
        code: 'deck_size',
      }),
    )
  }
  const limit = format === 'commander' ? 1 : 4
  // Copies of a card are counted across the zones the limit covers (Commander leaves the sideboard out); the issue sits
  // in the zone of its first line.
  const held = new Map<string, { zone: DeckZone; copies: number }>()
  for (const line of lines) {
    if (format === 'commander' && line.zone === 'sideboard') continue
    const found = held.get(line.oracle_id)
    held.set(line.oracle_id, { zone: found?.zone ?? line.zone, copies: (found?.copies ?? 0) + line.count })
  }
  for (const [oracleId, { zone, copies }] of held) {
    const name = nameOf(oracleId)
    if (isBasic(name) || copies <= limit) continue
    issues.push(
      legalityIssue(`${name}: ${copies} copies, but only ${limit} allowed`, {
        code: 'copies',
        zone,
        oracle_id: oracleId,
        name,
      }),
    )
  }
  for (const line of lines) {
    const name = nameOf(line.oracle_id)
    if (!BANNED.includes(name)) continue
    issues.push(
      legalityIssue(`${name} is banned in ${format}`, {
        code: 'banned',
        zone: line.zone,
        oracle_id: line.oracle_id,
        name,
      }),
    )
  }
  return issues
}

/** What the check endpoint of `/dev/builder` knows: the fixture cards, their clauses, its rules, and a pause to watch. */
export function fixtureCheck(): CheckScript {
  return {
    cards: fixtureCards(),
    reasons: Object.fromEntries(
      ALL.filter((entry) => entry.clauses !== undefined).map((entry) => [
        oracleIdOf(entry.name),
        reasonsOf(entry),
      ]),
    ),
    legality: fixtureLegality,
    delayMs: 450,
  }
}

/**
 * A Commander deck for `/dev/builder`. `mixed` holds a card of each support state, a card the game cannot host and
 * illegal lines (4 copies of a singleton card, a banned card, the wrong size); `manual` is legal with cards played by
 * hand; `verified` has only verified cards; `changed` and `unchecked` are the mixed cards with no valid check.
 */
export function fixtureView(deck: DevDeck): DeckView {
  const entries =
    deck === 'verified'
      ? CLEAN
      : deck === 'manual'
        ? [...CLEAN, ...BY_HAND]
        : [...CLEAN, ...BY_HAND, ...BLOCKING, ...ILLEGAL]
  const legal = deck === 'verified' || deck === 'manual'
  const lands = legal
    ? 99 - entries.filter((e) => (e.zone ?? 'main') === 'main').reduce((n, e) => n + e.count, 0)
    : 30
  const all: Entry[] = [...entries, { name: 'Mountain', count: lands, support: 'verified' }]
  const lines: DeckLineView[] = all.map((entry) => ({
    zone: entry.zone ?? 'main',
    count: entry.count,
    printing_id: null,
    card: hitOf(entry),
  }))
  const cards: ReportCard[] = all.map((entry) =>
    reportCard(entry.name, {
      zone: entry.zone ?? 'main',
      count: entry.count,
      support: entry.support,
      reasons: reasonsOf(entry),
    }),
  )
  const legality = fixtureLegality({
    deck: null,
    format: 'commander',
    cards: lines.map((line) => ({
      zone: line.zone,
      oracle_id: line.card.oracle_id,
      printing_id: null,
      count: line.count,
    })),
  })
  const report = reportOf(cards, legality)
  const stateOf: Record<DevDeck, DeckState> = {
    mixed: report.summary.state,
    manual: report.summary.state,
    verified: report.summary.state,
    changed: 'unchecked',
    unchecked: 'unchecked',
  }
  const state = stateOf[deck]
  return deckView({
    id: 'dev-deck',
    name: 'Krenko Goblins',
    format: 'commander',
    cards: lines,
    state,
    verified: state === 'verified',
    changed: deck === 'changed',
    report,
  })
}
