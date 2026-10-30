// Goal: a stack item's targets are listed once each in order, a target that is gone (the server says it is illegal, or without its word a player who lost, an object that is not in the view or left where it was since the item went on the stack) is marked, and a stack item among the targets is told apart.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, ObjectView, PlayerView, Target } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { hasNoLegalTarget, live, movedSince, stackEntries } from './stackTargets'

const obj = (id: number, extra: Partial<ObjectView> = {}): ObjectView => ({
  ...card({ id }),
  ...extra,
})
const on = (id: number): Target => ({ Object: { id, counter: 0 } })
const log = (...events: Event[]): LogEntry[] =>
  events.map((event, seq) => ({ seq, event: { Public: event } }))
const players = (lost: number[] = []): PlayerView[] =>
  [0, 1].map((seat) => ({ seat, lost: lost.includes(seat) ? 'Conceded' : null }) as unknown as PlayerView)

function entriesOf(item: ObjectView, others: ObjectView[], events: LogEntry[] = [], lost: number[] = []) {
  const objectsById = Object.fromEntries([item, ...others].map((o) => [o.id, o]))
  return stackEntries({ ids: [item.id], objectsById, players: players(lost), log: events })
}

describe('movedSince', () => {
  it('collects the objects that left after the item went on the stack and nothing before it', () => {
    const events = log(
      { ZoneChanged: { object: 7, from: 'Battlefield', to: 'Exile', cause: 'Exiled' } },
      { SpellCast: { player: 0, object: 1 } },
      { ZoneChanged: { object: 8, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'Destroyed' } },
      { SpellResolved: { object: 9 } },
      { ObjectLeftGame: { object: 10 } },
    )
    expect([...movedSince(events, 1)].sort((a, b) => a - b)).toEqual([8, 9, 10])
  })

  it('claims nothing when the log no longer holds the cast', () => {
    expect(
      movedSince(log({ ZoneChanged: { object: 8, from: 'Exile', to: 'Exile', cause: 'Moved' } }), 1).size,
    ).toBe(0)
  })
})

describe('stackEntries', () => {
  const bolt = (targets: Target[][]) => obj(1, { zone: 'Stack', controller: 0, targets })

  it('lists every target once, in the order they are named, across the target specifications', () => {
    const [entry] = entriesOf(bolt([[on(2), { Player: 1 }], [on(2)]]), [obj(2, { controller: 1 })])
    expect(entry?.targets.map((t) => t.key)).toEqual(['object:2:0', 'player:1'])
    expect(entry?.targets[0]).toMatchObject({ gone: false, onStack: false, seat: 1 })
  })

  it('marks a player who lost, an object the view does not hold and one that moved since the cast', () => {
    const item = bolt([[on(2), on(3), on(4), { Player: 1 }]])
    const events = log(
      { SpellCast: { player: 0, object: 1 } },
      { ZoneChanged: { object: 3, from: 'Battlefield', to: { Graveyard: 1 }, cause: 'Destroyed' } },
    )
    const [entry] = entriesOf(item, [obj(2), obj(3), obj(4)], events, [1])
    expect(entry?.targets.map((t) => [t.key, t.gone])).toEqual([
      ['object:2:0', false],
      ['object:3:0', true],
      ['object:4:0', false],
      ['player:1', true],
    ])
    const missing = entriesOf(bolt([[on(5)]]), [])[0]
    expect(missing?.targets[0]?.gone).toBe(true)
  })

  it('tells a stack item among the targets from a card', () => {
    const [entry] = entriesOf(bolt([[on(2)]]), [obj(2, { zone: 'Stack', controller: 1 })])
    expect(entry?.targets[0]?.onStack).toBe(true)
  })

  it('says an item has no legal target only when it aims at something and all of it is gone', () => {
    const none = entriesOf(bolt([]), [])[0]
    const gone = entriesOf(bolt([[on(5)]]), [])[0]
    const kept = entriesOf(bolt([[on(2)]]), [obj(2)])[0]
    expect([none, gone, kept].map((e) => (e === undefined ? null : hasNoLegalTarget(e)))).toEqual([
      false,
      true,
      false,
    ])
    expect(kept === undefined ? [] : live(kept)).toHaveLength(1)
  })
})

describe('targets_legal', () => {
  const cast = log({ SpellCast: { player: 0, object: 1 } })
  const moved = log(
    { SpellCast: { player: 0, object: 1 } },
    { ZoneChanged: { object: 2, from: 'Battlefield', to: 'Exile', cause: 'Exiled' } },
  )
  it.each([
    {
      name: 'a legal object the log never saw move',
      legal: [[true]],
      events: cast,
      gone: [false],
      fizzles: false,
    },
    {
      name: 'an object made illegal without moving (hexproof)',
      legal: [[false]],
      events: cast,
      gone: [true],
      fizzles: true,
    },
    {
      name: 'the server over the log for an object that moved',
      legal: [[true]],
      events: moved,
      gone: [false],
      fizzles: false,
    },
    {
      name: 'the log when the view does not say',
      legal: undefined,
      events: moved,
      gone: [true],
      fizzles: true,
    },
  ])('marks a target by $name', ({ legal, events, gone, fizzles }) => {
    const item = obj(1, {
      zone: 'Stack',
      targets: [[on(2)]],
      ...(legal === undefined ? {} : { targets_legal: legal }),
    })
    const [entry] = entriesOf(item, [obj(2, { controller: 1 })], events)
    expect(entry?.targets.map((t) => t.gone)).toEqual(gone)
    expect(entry === undefined ? null : hasNoLegalTarget(entry)).toBe(fizzles)
    expect(entry?.targets[0]?.seat).toBe(1)
  })

  it('keeps a target named twice legal while one of its specifications still finds it', () => {
    const item = obj(1, {
      zone: 'Stack',
      targets: [[on(2)], [on(2)], [{ Player: 1 }]],
      targets_legal: [[false], [true], [false]],
    })
    const [entry] = entriesOf(item, [obj(2)])
    expect(entry?.targets.map((t) => [t.key, t.gone])).toEqual([
      ['object:2:0', false],
      ['player:1', true],
    ])
  })
})
