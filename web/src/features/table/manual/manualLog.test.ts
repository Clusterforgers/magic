// Goal: every manual action reads as one log sentence ending "(by hand)", a moved card names the zone it left, the events that carry the change are not told twice, what follows them (deaths, triggers) stays, and a viewer who cannot see the card gets the same sentence without its name.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, EventView, ManualAction, PlayerId } from '@/protocol/generated'
import { hid, pub } from '@/test-utils/turnEvents'
import { describeEvent } from '../eventText'
import { foldLog } from '../logFold'

const NAMES = ['Ana', 'Bo']
const CARDS: Record<number, string> = { 1: 'Sol Ring', 2: 'Grizzly Bears', 3: 'Sword', 4: 'Forest' }
const names = {
  seat: (seat: PlayerId) => NAMES[seat] ?? `Seat ${seat}`,
  card: (id: number) => CARDS[id] ?? 'a card',
}

const action = (what: ManualAction, player: PlayerId = 1): EventView =>
  pub({ ManualAction: { player, action: what } })
const log = (events: readonly EventView[]): LogEntry[] => events.map((event, seq) => ({ seq, event }))
const fold = (events: readonly EventView[]): string[] =>
  foldLog(log(events), names, { seats: [0, 1] }).map((line) => line.text)

describe('the sentence of each manual action', () => {
  const sentences: [string, ManualAction, string][] = [
    ['a move', { Move: { object: 1, to: 'Graveyard' } }, 'Bo moves Sol Ring to the graveyard (by hand)'],
    ['a move to the hand', { Move: { object: 1, to: 'Hand' } }, 'Bo moves Sol Ring to the hand (by hand)'],
    ['a move to exile', { Move: { object: 1, to: 'Exile' } }, 'Bo moves Sol Ring to exile (by hand)'],
    [
      'a move to the battlefield',
      { Move: { object: 1, to: 'Battlefield' } },
      'Bo moves Sol Ring to the battlefield (by hand)',
    ],
    [
      'a move to the top of the library',
      { Move: { object: 1, to: 'LibraryTop' } },
      'Bo moves Sol Ring to the top of the library (by hand)',
    ],
    [
      'a move to the bottom of the library',
      { Move: { object: 1, to: 'LibraryBottom' } },
      'Bo moves Sol Ring to the bottom of the library (by hand)',
    ],
    [
      'a move to the command zone',
      { Move: { object: 1, to: 'Command' } },
      'Bo moves Sol Ring to the command zone (by hand)',
    ],
    [
      'the top cards moved',
      { MoveTop: { count: 3, to: 'Graveyard' } },
      'Bo moves the top 3 cards of their library to the graveyard (by hand)',
    ],
    [
      'the top card moved',
      { MoveTop: { count: 1, to: 'Hand' } },
      'Bo moves the top 1 card of their library to the hand (by hand)',
    ],
    [
      'a counter put on',
      { Counters: { object: 2, counter: '+1/+1', delta: 1 } },
      'Bo puts a +1/+1 counter on Grizzly Bears (by hand)',
    ],
    [
      'counters taken off',
      { Counters: { object: 2, counter: { Other: 'oil' }, delta: -3 } },
      'Bo removes 3 oil counters from Grizzly Bears (by hand)',
    ],
    [
      'player counters gained',
      { PlayerCounters: { player: 1, counter: 'poison', delta: 2 } },
      'Bo gets 2 poison counters (by hand)',
    ],
    [
      'player counters lost',
      { PlayerCounters: { player: 1, counter: 'poison', delta: -1 } },
      'Bo loses a poison counter (by hand)',
    ],
    ['life gained', { Life: { player: 1, delta: 5 } }, 'Bo gains 5 life (by hand)'],
    ['life lost', { Life: { player: 1, delta: -3 } }, 'Bo loses 3 life (by hand)'],
    [
      'a common token',
      { CreateToken: { token: { Common: 'Treasure' }, count: 2 } },
      'Bo creates 2 Treasure tokens (by hand)',
    ],
    [
      'a custom token',
      {
        CreateToken: {
          token: {
            Custom: { name: 'Knight', colors: [], types: ['Creature'], subtypes: [], power: 2, toughness: 2 },
          },
          count: 1,
        },
      },
      'Bo creates 1 Knight token (by hand)',
    ],
    ['a tap', { Tap: { object: 4 } }, 'Bo taps Forest (by hand)'],
    ['an untap', { Untap: { object: 4 } }, 'Bo untaps Forest (by hand)'],
    ['a draw', { Draw: { count: 2 } }, 'Bo draws 2 cards (by hand)'],
    ['a discard', { Discard: { object: 1 } }, 'Bo discards Sol Ring (by hand)'],
    ['an attach', { Attach: { object: 3, to: 2 } }, 'Bo attaches Sword to Grizzly Bears (by hand)'],
    ['an unattach', { Unattach: { object: 3 } }, 'Bo takes Sword off what it is attached to (by hand)'],
  ]
  it.each(sentences)('%s', (_name, what, text) => {
    const line = describeEvent(action(what), names)
    expect(line).toEqual({ text, minor: false })
    expect(fold([action(what)])).toEqual([text])
  })

  it('names the seat that acted', () => {
    expect(describeEvent(action({ Tap: { object: 4 } }, 0), names).text).toBe('Ana taps Forest (by hand)')
  })

  it('tells a move the viewer cannot name without the name', () => {
    const line = describeEvent(hid({ ManualMove: { player: 1, to: 'LibraryTop' } }), names)
    expect(line.text).toBe('Bo moves a card to the top of the library (by hand)')
  })
})

describe('a move names the zone the card left', () => {
  const left = (from: Event extends infer E ? (E extends { ZoneChanged: infer Z } ? Z : never) : never) =>
    pub({ ZoneChanged: from })

  it.each([
    ['the battlefield', 'Battlefield', 'Graveyard', 'the battlefield'],
    ['the hand', { Hand: 1 }, 'Graveyard', 'the hand'],
    ['the graveyard', { Graveyard: 1 }, 'Exile', 'the graveyard'],
    ['exile', 'Exile', 'Hand', 'exile'],
    ['the command zone', { Command: 1 }, 'Battlefield', 'the command zone'],
  ] as const)('from %s', (_name, from, to, words) => {
    const zones = {
      Graveyard: { Graveyard: 1 },
      Exile: 'Exile',
      Hand: { Hand: 1 },
      Battlefield: 'Battlefield',
    } as const
    const lines = fold([
      action({ Move: { object: 1, to } }),
      left({ object: 1, from, to: zones[to], cause: 'Moved' }),
    ])
    expect(lines).toHaveLength(1)
    expect(lines[0]).toMatch(new RegExp(`^Bo moves Sol Ring from ${words} to `))
    expect(lines[0]).toMatch(/\(by hand\)$/)
  })

  it('tells the example of the roadmap in full', () => {
    expect(
      fold([
        action({ Move: { object: 1, to: 'Graveyard' } }),
        left({ object: 1, from: 'Battlefield', to: { Graveyard: 1 }, cause: 'Moved' }),
      ]),
    ).toEqual(['Bo moves Sol Ring from the battlefield to the graveyard (by hand)'])
  })
})

describe('the events of the change', () => {
  const zone = (
    object: number,
    from: 'Battlefield' | { Hand: number },
    to: 'Exile' | { Graveyard: number },
  ) => pub({ ZoneChanged: { object, from, to, cause: 'Moved' } })

  it('are told once, in the sentence, and the log goes on after them', () => {
    const events = [
      pub({ LifeLost: { player: 0, amount: 1 } }),
      action({ Counters: { object: 2, counter: '+1/+1', delta: 1 } }),
      pub({ CountersChanged: { on: { Object: { id: 2, counter: 0 } }, counter: '+1/+1', delta: 1 } }),
      pub({ LifeGained: { player: 0, amount: 2 } }),
    ]
    expect(fold(events)).toEqual([
      'Ana loses 1 life',
      'Bo puts a +1/+1 counter on Grizzly Bears (by hand)',
      'Ana gains 2 life',
    ])
  })

  it('leave in what a change set off, such as the life a creature put into the graveyard costs', () => {
    const events = [
      action({ Move: { object: 2, to: 'Graveyard' } }),
      zone(2, 'Battlefield', { Graveyard: 1 }),
      pub({ LifeLost: { player: 0, amount: 1 } }),
    ]
    expect(fold(events)).toEqual([
      'Bo moves Grizzly Bears from the battlefield to the graveyard (by hand)',
      'Ana loses 1 life',
    ])
  })

  it.each([
    [
      'a draw',
      { Draw: { count: 2 } },
      [pub({ CardDrawn: { player: 1, object: 1 } }), pub({ CardDrawn: { player: 1, object: 2 } })],
    ],
    [
      'the top cards moved',
      { MoveTop: { count: 2, to: 'Graveyard' } },
      [zone(1, { Hand: 1 }, { Graveyard: 1 }), zone(2, { Hand: 1 }, { Graveyard: 1 })],
    ],
    ['a tap', { Tap: { object: 4 } }, [pub({ PermanentTapped: { object: 4 } })]],
    ['an untap', { Untap: { object: 4 } }, [pub({ PermanentUntapped: { object: 4 } })]],
    ['life gained', { Life: { player: 1, delta: 2 } }, [pub({ LifeGained: { player: 1, amount: 2 } })]],
    ['life lost', { Life: { player: 1, delta: -2 } }, [pub({ LifeLost: { player: 1, amount: 2 } })]],
    ['an attach', { Attach: { object: 3, to: 2 } }, [pub({ Attached: { object: 3, to: 2 } })]],
    ['an unattach', { Unattach: { object: 3 } }, [pub({ Unattached: { object: 3 } })]],
    [
      'a token',
      { CreateToken: { token: { Common: 'Treasure' }, count: 1 } },
      [pub({ TokenCreated: { object: 9, controller: 1, name: 'Treasure' } })],
    ],
    [
      'a discard',
      { Discard: { object: 1 } },
      [pub({ CardDiscarded: { player: 1, object: 1 } }), zone(1, { Hand: 1 }, { Graveyard: 1 })],
    ],
  ] as [string, ManualAction, EventView[]][])('of %s fold into one line', (_name, what, change) => {
    expect(fold([action(what), ...change])).toHaveLength(1)
  })

  it('two actions in a row are two lines', () => {
    const events = [
      action({ Tap: { object: 4 } }),
      pub({ PermanentTapped: { object: 4 } }),
      action({ Untap: { object: 4 } }),
      pub({ PermanentUntapped: { object: 4 } }),
    ]
    expect(fold(events)).toEqual(['Bo taps Forest (by hand)', 'Bo untaps Forest (by hand)'])
  })

  it('stay as lines when no action told them, as before', () => {
    expect(fold([pub({ PermanentTapped: { object: 4 } })])).toEqual(['Forest becomes tapped'])
  })

  it('marks the line of an action and no other', () => {
    const lines = foldLog(
      log([pub({ LifeLost: { player: 0, amount: 1 } }), action({ Tap: { object: 4 } })]),
      names,
      { seats: [0, 1] },
    )
    expect(lines.map((line) => line.manual)).toEqual([undefined, true])
    expect(lines.every((line) => !line.minor)).toBe(true)
  })
})

describe('a viewer who cannot see the card', () => {
  it('reads the move from its hidden twin and the zone it left', () => {
    const events = [
      hid({ ManualMove: { player: 1, to: 'LibraryTop' } }),
      hid({ ZoneChanged: { from: { Hand: 1 }, to: { Library: 1 }, cause: 'Moved' } }),
    ]
    expect(fold(events)).toEqual(['Bo moves a card from the hand to the top of the library (by hand)'])
  })

  it('reads a hidden move with no change after it on its own', () => {
    expect(fold([hid({ ManualMove: { player: 1, to: 'Hand' } })])).toEqual([
      'Bo moves a card to the hand (by hand)',
    ])
  })

  it('folds a draw told as hidden card draws', () => {
    const events = [
      action({ Draw: { count: 2 } }),
      hid({ CardDrawn: { player: 1 } }),
      hid({ CardDrawn: { player: 1 } }),
    ]
    expect(fold(events)).toEqual(['Bo draws 2 cards (by hand)'])
  })
})
