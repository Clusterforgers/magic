// Goal: the log and the ticker name commander damage: only combat damage of a commander to a player counts (dealt at the start of a combat damage step, before priority), each hit carries the total it brought the player to, counted back from the view so a log cut short still adds up, and the folded log and the ticker read "Atraxa deals 7 commander damage to Bo (14/21)".
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, EventView, PlayerView } from '@/protocol/generated'
import { commanderHits } from './commanderLog'
import { describeEvent } from './eventText'
import { foldLog } from './logFold'

const ATRAXA = 10
const KRENKO = 20
const BEAR = 30

const player = (seat: number, extra: Partial<PlayerView> = {}): PlayerView => ({
  seat,
  life: 40,
  lost: null,
  mana: { counts: [0, 0, 0, 0, 0, 0] },
  mulligans: 0,
  mulligan: 'Kept',
  hand: { count: 0, cards: null },
  library: { count: 0, cards: null },
  graveyard: { count: 0, cards: [] },
  command: { count: 0, cards: [] },
  bank: 0,
  ...extra,
})

const PLAYERS: PlayerView[] = [
  player(0, { commanders: [{ object: ATRAXA, casts: 1, tax: 2 }] }),
  player(1, {
    commanders: [{ object: KRENKO, casts: 0, tax: 0 }],
    commander_damage: [{ commander: ATRAXA, damage: 14 }],
    commander_lethal: 21,
  }),
]

const pub = (event: Event): EventView => ({ Public: event })
const damageStep: EventView = pub({ StepBegan: { step: 'CombatDamage' } })
const granted: EventView = pub({ PriorityGranted: { player: 0 } })
const hit = (source: number, amount: number, seat = 1): EventView =>
  pub({ DamageDealt: { source, target: { Player: seat }, amount } })

describe('commander hits', () => {
  it.each<[string, EventView[], [number, number, number][]]>([
    ['one hit in the damage step', [damageStep, hit(ATRAXA, 7)], [[1, 7, 14]]],
    ['two hits count back from the view', [damageStep, hit(ATRAXA, 7), granted, damageStep, hit(ATRAXA, 7)], [[1, 7, 7], [4, 7, 14]]],
    ['a creature that is no commander', [damageStep, hit(BEAR, 3)], []],
    ['a commander after priority (an ability)', [damageStep, granted, hit(ATRAXA, 2)], []],
    ['a commander outside combat', [pub({ StepBegan: { step: 'Main1' } }), hit(ATRAXA, 2)], []],
    ['a hit to a creature', [damageStep, pub({ DamageDealt: { source: ATRAXA, target: { Object: BEAR }, amount: 3 } })], []],
    ['first strike damage', [pub({ StepBegan: { step: 'FirstStrikeDamage' } }), hit(ATRAXA, 4)], [[1, 4, 14]]],
  ])('%s', (_, events, expected) => {
    const hits = commanderHits(events, PLAYERS)
    expect([...hits].map(([index, h]) => [index, h.amount, h.total])).toEqual(expected)
    for (const h of hits.values()) expect(h.lethal).toBe(21)
  })

  it('finds nothing at a table without commanders', () => {
    expect(commanderHits([damageStep, hit(ATRAXA, 7)], [player(0), player(1)]).size).toBe(0)
  })

  it('a batch that starts while damage is dealt counts from its first event', () => {
    expect(commanderHits([hit(ATRAXA, 7)], PLAYERS, true).get(0)?.total).toBe(14)
  })
})

describe('the lines of the return choice', () => {
  const names = { seat: () => 'Alice', card: () => 'Atraxa' }
  it.each<[Event, string]>([
    [{ ZoneChanged: { object: ATRAXA, from: { Graveyard: 0 }, to: { Command: 0 }, cause: 'Moved' } }, 'Atraxa returns to the command zone'],
    [{ ZoneChanged: { object: ATRAXA, from: 'Battlefield', to: { Command: 0 }, cause: 'Destroyed' } }, 'Atraxa is destroyed and returns to the command zone'],
    [{ ZoneChanged: { object: ATRAXA, from: 'Battlefield', to: { Command: 0 }, cause: 'StateBased' } }, 'Atraxa returns to the command zone'],
    [{ CommanderStayed: { object: ATRAXA } }, 'Atraxa stays out of the command zone'],
  ])('%j reads %s', (event, text) => {
    expect(describeEvent({ Public: event }, names).text).toBe(text)
  })
})

describe('the folded log', () => {
  const names = {
    seat: (seat: number) => ['Alice', 'Bo'][seat] ?? 'someone',
    card: (id: number) => (id === ATRAXA ? 'Atraxa' : id === KRENKO ? 'Krenko' : 'Bear'),
  }
  const entries = (events: EventView[]): LogEntry[] => events.map((event, seq) => ({ seq, event }))

  it.each<[string, PlayerView[], string]>([
    ['with a lethal amount', PLAYERS, 'Atraxa deals 7 commander damage to Bo (14/21)'],
    [
      'with the rule off',
      PLAYERS.map((p) => ({ ...p, commander_lethal: undefined })),
      'Atraxa deals 7 commander damage to Bo (14)',
    ],
  ])('names a hit %s', (_, players, text) => {
    const lines = foldLog(entries([damageStep, hit(ATRAXA, 7)]), names, { seats: [0, 1], players })
    expect(lines.map((line) => line.text)).toContain(text)
  })

  it('reads other damage as before', () => {
    const lines = foldLog(entries([damageStep, hit(BEAR, 3)]), names, { seats: [0, 1], players: PLAYERS })
    expect(lines.map((line) => line.text)).toContain('Bear deals 3 damage to Bo')
  })
})
