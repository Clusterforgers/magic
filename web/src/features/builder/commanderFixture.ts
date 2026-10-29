/** Cards and a deck-check script for tests of the commander slot: the report's commander rules over a few known cards. */
import { cardHit } from '@/features/decks/fakeDecks'
import type { CardColor, CardHit, CheckRequest, Issue } from '@/protocol/generated'
import type { CheckScript } from './fakeCheck'

const legend = (name: string, identity: CardColor[], over: Partial<CardHit> = {}): CardHit =>
  cardHit(name, {
    type_line: 'Legendary Creature — Human',
    mana_cost: '{2}',
    colors: identity,
    color_identity: identity,
    oracle_id: `oracle-${name.toLowerCase().replace(/\W+/g, '-')}`,
    ...over,
  })

/** A legendary creature with no pairing rule. */
export const KRENKO = legend('Krenko, Mob Boss', ['R'])
/** Two cards that say Partner. */
export const TYMNA = legend('Tymna the Weaver', ['W', 'B'])
export const THRASIOS = legend('Thrasios, Triton Hero', ['U', 'G'])
/** A creature that chooses a Background, and the Background. */
export const WILSON = legend('Wilson, Dungeon Master', ['R'])
export const GIANTS = legend('Raised by Giants', ['G'], { type_line: 'Legendary Enchantment — Background' })
/** A card that is no commander. */
export const BEAR = cardHit('Grizzly Bears', {
  type_line: 'Creature — Bear',
  colors: ['G'],
  color_identity: ['G'],
  oracle_id: 'oracle-bears',
})
/** A red card and a white one, to put in a deck led by a red or a white commander. */
export const FIREBALL = cardHit('Fireball', { type_line: 'Sorcery', oracle_id: 'oracle-fireball' })
export const PIKEMAN = cardHit('Pikeman', {
  type_line: 'Creature — Human',
  colors: ['W'],
  color_identity: ['W'],
  oracle_id: 'oracle-pikeman',
})

/** Every card of the fixture. */
export const COMMANDER_POOL: CardHit[] = [KRENKO, TYMNA, THRASIOS, WILSON, GIANTS, BEAR, FIREBALL, PIKEMAN]

const KIND: Readonly<Record<string, 'solo' | 'partner' | 'chooser' | 'background'>> = {
  [KRENKO.oracle_id]: 'solo',
  [TYMNA.oracle_id]: 'partner',
  [THRASIOS.oracle_id]: 'partner',
  [WILSON.oracle_id]: 'chooser',
  [GIANTS.oracle_id]: 'background',
}

const issue = (code: Issue['code'], message: string, over: Partial<Issue> = {}): Issue => ({
  code,
  message,
  zone: null,
  oracle_id: null,
  name: null,
  ...over,
})

/** The commander rules of `docs/deck-check.md` over the cards above: who may lead, who pairs, what lies outside. */
export function commanderLegality(request: CheckRequest): Issue[] {
  const lines = request.cards ?? []
  const known = new Map(COMMANDER_POOL.map((card) => [card.oracle_id, card]))
  const commanders = lines.filter((line) => line.zone === 'commander').map((line) => line.oracle_id)
  const issues: Issue[] = []
  if (commanders.length > 2) {
    issues.push(
      issue(
        'commander_count',
        `a Commander deck has one commander, or two that can be partnered; this one has ${commanders.length}`,
        { zone: 'commander' },
      ),
    )
  }
  for (const id of commanders) {
    const name = known.get(id)?.name ?? id
    if (KIND[id] === undefined || (KIND[id] === 'background' && commanders.length < 2)) {
      issues.push(
        issue(
          'commander_eligibility',
          `${name} cannot be a commander: it is no legendary creature and does not say it can`,
          {
            zone: 'commander',
            oracle_id: id,
            name,
          },
        ),
      )
    }
  }
  const [a, b] = commanders.map((id) => KIND[id])
  const pairs =
    (a === 'partner' && b === 'partner') ||
    (a === 'chooser' && b === 'background') ||
    (a === 'background' && b === 'chooser')
  if (commanders.length === 2 && !pairs) {
    issues.push(
      issue(
        'commander_pair',
        'these two commanders cannot be partnered: both need Partner, Friends forever, or a Partner with each other, or a Background with Choose a Background',
        { zone: 'commander' },
      ),
    )
  }
  const identity = new Set(commanders.flatMap((id) => known.get(id)?.color_identity ?? []))
  if (commanders.length > 0) {
    for (const line of lines.filter((l) => l.zone === 'main')) {
      const card = known.get(line.oracle_id)
      const outside = (card?.color_identity ?? []).filter((color) => !identity.has(color))
      if (card !== undefined && outside.length > 0) {
        issues.push(
          issue(
            'color_identity',
            `${card.name} has ${outside.join('')} in its color identity, outside the commander's (${[...identity].join('')})`,
            { zone: 'main', oracle_id: card.oracle_id, name: card.name },
          ),
        )
      }
    }
  }
  return issues
}

/** The check script of a Commander deck builder test. */
export const COMMANDER_CHECK: CheckScript = { cards: COMMANDER_POOL, legality: commanderLegality }
