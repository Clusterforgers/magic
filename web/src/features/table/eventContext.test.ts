// Goal: the log names what a play aimed at and what did it: a cast and an activation list their targets ("A and B", "A, B and C"), a counter names the spell that counters, an exile, a destroy and a bounce name the resolving spell, and every line stays as it was when the event has nothing more to say.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, EventView, Target } from '@/protocol/generated'
import { eventContexts } from './eventContext'
import { describeEvent } from './eventText'
import type { EventNames } from './eventText'
import { foldLog } from './logFold'

const CARDS: Record<number, string> = {
  1: 'Grizzly Bears',
  2: 'Hill Giant',
  3: 'Forest',
  4: 'Lightning Bolt',
  5: 'Counterspell',
  6: 'Swords to Plowshares',
  7: 'Gravedigger',
  8: 'Doom Blade',
  9: 'Unsummon',
}
const names: EventNames = {
  seat: (seat) => ['Ana', 'Bo', 'Cy'][seat] ?? `Seat ${seat + 1}`,
  card: (id) => CARDS[id] ?? 'a card',
}
const on = (id: number): Target => ({ Object: { id, counter: 0 } })
const view = (events: Event[]): EventView[] => events.map((event) => ({ Public: event }))
const lines = (events: Event[]): string[] => {
  const views = view(events)
  const contexts = eventContexts(views)
  return views.map((event, index) => describeEvent(event, names, contexts[index]).text)
}
const cast = (targets?: Target[][]): Event => ({
  SpellCast: { player: 0, object: 4, ...(targets === undefined ? {} : { choices: { targets } }) },
})

describe('casting at targets', () => {
  it.each([
    ['no targets', undefined, 'Ana casts Lightning Bolt'],
    ['an empty list', [], 'Ana casts Lightning Bolt'],
    ['one card', [[on(2)]], 'Ana casts Lightning Bolt targeting Hill Giant'],
    ['a card and a player', [[on(2), { Player: 1 }]], 'Ana casts Lightning Bolt targeting Hill Giant and Bo'],
    [
      'three, over two specifications',
      [[on(2), on(1)], [{ Player: 2 }]],
      'Ana casts Lightning Bolt targeting Hill Giant, Grizzly Bears and Cy',
    ],
    ['one target named twice', [[on(2)], [on(2)]], 'Ana casts Lightning Bolt targeting Hill Giant'],
  ])('says %s', (_, targets, text) => {
    expect(lines([cast(targets)])).toEqual([text])
  })
})

describe('activating and triggering at targets', () => {
  const source = { id: 7, counter: 0 }
  const put = (targets: Target[][], extra: object = {}): Event => ({
    AbilityPutOnStack: {
      object: 90,
      source,
      snapshot: {} as never,
      ability: 0,
      controller: 0,
      choices: { targets },
      ...extra,
    },
  })
  const activated: Event = { AbilityActivated: { player: 0, source, ability: 0 } }

  it('names the targets in the activation line from the ability that goes on the stack next', () => {
    expect(lines([activated, { ManaSpent: { player: 0, mana: ['Black'] } }, put([[on(1)]])])).toEqual([
      'Ana activates an ability of Gravedigger targeting Grizzly Bears',
      'Ana spends {B}',
      'An ability of Gravedigger goes on the stack',
    ])
  })

  it('keeps the activation line as it was when the ability has no target or the log ends first', () => {
    expect(lines([activated, put([])])[0]).toBe('Ana activates an ability of Gravedigger')
    expect(lines([activated])[0]).toBe('Ana activates an ability of Gravedigger')
    expect(lines([activated, { PriorityGranted: { player: 0 } }, put([[on(1)]])])[0]).toBe(
      'Ana activates an ability of Gravedigger',
    )
  })

  it('names the targets of a triggered ability in its put-on-stack line, which is then not quiet', () => {
    const [triggered] = view([put([[on(2)]], { cause: { subjects: [], snapshots: [] } })])
    const line = describeEvent(triggered as EventView, names)
    expect(line).toEqual({
      text: 'An ability of Gravedigger goes on the stack targeting Hill Giant',
      minor: false,
    })
  })
})

describe('what a resolving spell does', () => {
  const resolving = (...inside: Event[]): Event[] => [
    { ResolutionBegan: { object: 6 } },
    ...inside,
    { ResolutionEnded: { object: 6, outcome: 'Resolved' } },
  ]

  it('names the spell that counters', () => {
    expect(
      lines([
        { ResolutionBegan: { object: 5 } },
        { ResolutionEnded: { object: 4, outcome: 'Countered' } },
        { ResolutionEnded: { object: 5, outcome: 'Resolved' } },
      ]),
    ).toEqual([
      'Counterspell begins to resolve',
      'Counterspell counters Lightning Bolt',
      'Counterspell finishes resolving',
    ])
  })

  it('keeps "is countered" for an object countered outside a resolution or by its own end', () => {
    expect(lines([{ ResolutionEnded: { object: 4, outcome: 'Countered' } }])).toEqual([
      'Lightning Bolt is countered',
    ])
    expect(
      lines([
        { ResolutionBegan: { object: 4 } },
        { ResolutionEnded: { object: 4, outcome: 'Countered' } },
      ])[1],
    ).toBe('Lightning Bolt is countered')
  })

  it.each([
    ['Exiled', 'Hill Giant is exiled by Swords to Plowshares'],
    ['Destroyed', 'Hill Giant is destroyed by Swords to Plowshares'],
    ['Buried', "Hill Giant is destroyed by Swords to Plowshares and can't be regenerated"],
  ] as const)('names the spell that makes a move with cause %s', (cause, text) => {
    expect(lines(resolving({ ZoneChanged: { object: 2, from: 'Battlefield', to: 'Exile', cause } }))[1]).toBe(
      text,
    )
  })

  it('says who returned a permanent to a hand, and keeps the plain line for a move nothing caused', () => {
    const bounce: Event = { ZoneChanged: { object: 2, from: 'Battlefield', to: { Hand: 1 }, cause: 'Moved' } }
    expect(lines(resolving(bounce))[1]).toBe("Swords to Plowshares returns Hill Giant to Bo's hand")
    expect(lines([bounce])).toEqual(["Hill Giant moves to Bo's hand"])
    const discard: Event = {
      ZoneChanged: { object: 2, from: { Hand: 1 }, to: { Graveyard: 1 }, cause: 'Discarded' },
    }
    expect(lines(resolving(discard))[1]).toBe('Hill Giant is discarded')
  })

  it('leaves the resolving spell itself and the lines after its end as they were', () => {
    const own: Event = { ZoneChanged: { object: 6, from: 'Stack', to: 'Exile', cause: 'Exiled' } }
    expect(lines(resolving(own))[1]).toBe('Swords to Plowshares is exiled')
    const after: Event = { ZoneChanged: { object: 2, from: 'Battlefield', to: 'Exile', cause: 'Exiled' } }
    expect(lines([...resolving(), after]).at(-1)).toBe('Hill Giant is exiled')
  })
})

describe('the context', () => {
  it('has one entry per event and none for an event outside any resolution', () => {
    const events: Event[] = [
      cast(),
      { ResolutionBegan: { object: 6 } },
      { LifeGained: { player: 0, amount: 1 } },
    ]
    expect(eventContexts(view(events))).toEqual([undefined, undefined, { resolving: 6 }])
  })

  it('sees only public events: a hidden one never carries a context', () => {
    expect(eventContexts([{ Hidden: { CardDrawn: { player: 0 } } }])).toEqual([undefined])
  })
})

describe('the folded log', () => {
  it('uses the contexts, so the panel reads the same words', () => {
    const log: LogEntry[] = view([
      { ResolutionBegan: { object: 8 } },
      { ZoneChanged: { object: 2, from: 'Battlefield', to: { Graveyard: 1 }, cause: 'Destroyed' } },
      { ResolutionEnded: { object: 8, outcome: 'Resolved' } },
    ]).map((event, seq) => ({ seq, event }))
    expect(foldLog(log, names, { seats: [0, 1] }).map((line) => line.text)).toContain(
      'Hill Giant is destroyed by Doom Blade',
    )
  })
})
