/** Goal: when a play the viewer began is taken back before the stack, the centre strip says which card and why in plain words; the note is about the viewer only, lasts until the viewer has been asked something and answered it, and goes once something is put on the stack or the turn changes. */
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { appendLog, initialPlayState } from '@/play/store'
import type { EventView, ObjectView, Unplayable } from '@/protocol/generated'
import { decisionFor } from '@/test-utils/decisions'
import { renderTable } from '@/test-utils/tableHarness'
import { latestRewound } from './rewound'

const rewound = (player: number, object: number, reason: Unplayable): EventView => ({
  Public: { AnnouncementRewound: { player, object, reason } },
})
const turn: EventView = { Public: { TurnBegan: { number: 6, active: 1 } } }
const cast: EventView = { Public: { SpellCast: { player: 1, object: 9 } } } as EventView
const passed: EventView = { Public: { PriorityPassed: { player: 0 } } }
const hiddenRewind: EventView = { Hidden: { AnnouncementRewound: { player: 0 } } }

const logOf = (...events: EventView[]): PlayState['log'] => appendLog([], 100, events)
const named = { 7: { id: 7, name: 'Imprisoned in the Moon' } as ObjectView }

describe('latestRewound', () => {
  const state = (log: PlayState['log'], seat: number | null = 0): PlayState => ({
    ...initialPlayState(),
    log,
    seat,
    objectsById: named,
  })

  it.each([
    ['an empty log', [], null],
    [
      'a rewind for mana',
      [rewound(0, 7, 'Mana')],
      'Imprisoned in the Moon could not be played: not enough mana',
    ],
    [
      'a rewind for targets',
      [passed, rewound(0, 7, 'Targets')],
      'Imprisoned in the Moon could not be played: no legal target',
    ],
    [
      'a card the viewer cannot see',
      [rewound(0, 8, 'Cost')],
      'That card could not be played: a cost cannot be paid',
    ],
    ['a rewind of another seat', [rewound(1, 7, 'Mana')], null],
    ['the hidden form another seat gets', [hiddenRewind], null],
    ['a rewind before a cast', [rewound(0, 7, 'Mana'), cast], null],
    ['a rewind before the turn changed', [rewound(0, 7, 'Mana'), turn], null],
  ] as const)('reads %s', (_name, events, expected) => {
    const note = latestRewound(state(logOf(...events)))
    expect(note?.text ?? null).toBe(expected)
  })

  it.each<[string, EventView, string]>([
    [
      'a cast short of mana with its total',
      {
        Public: {
          AnnouncementRewound: {
            player: 0,
            object: 7,
            reason: 'Mana',
            why: { Mana: { cost: [{ Generic: 4 }, { Colored: 'Green' }, { Colored: 'Green' }], at_hand: 5 } },
          },
        },
      },
      'Imprisoned in the Moon could not be played. Needs {4}{G}{G}: you have 5 mana available.',
    ],
    [
      'an activation short of mana',
      {
        Public: {
          AnnouncementRewound: {
            player: 0,
            object: 7,
            reason: 'Mana',
            why: { Mana: { cost: [{ Generic: 3 }, { Colored: 'Black' }], at_hand: 2 } },
            activated: true,
          },
        },
      },
      'Imprisoned in the Moon could not be activated. Needs {3}{B}: you have 2 mana available.',
    ],
    [
      'an activation with no detail',
      { Public: { AnnouncementRewound: { player: 0, object: 7, reason: 'Cost', activated: true } } },
      'Imprisoned in the Moon could not be activated: a cost cannot be paid',
    ],
    [
      'a cast with no host',
      {
        Public: {
          AnnouncementRewound: {
            player: 0,
            object: 7,
            reason: 'Targets',
            why: { Host: { needs: 'a creature on the battlefield', by: 'Enchant' } },
          },
        },
      },
      'Imprisoned in the Moon could not be played. Imprisoned in the Moon needs a creature on the battlefield to enchant.',
    ],
  ])('says the detail of %s', (_name, event, expected) => {
    expect(latestRewound(state(logOf(event)))?.text).toBe(expected)
  })

  it('finds nothing when the seat is not known', () => {
    expect(latestRewound(state(logOf(rewound(0, 7, 'Mana')), null))).toBeNull()
  })
})

describe('the rewind note in the centre strip', () => {
  it('shows in the live region and clears once the next question is answered', () => {
    const table = renderTable({ state: (state) => ({ ...state, log: [], pending: null }) })
    const note = (): HTMLElement | null => table.container.querySelector('[data-slot="rewound"]')
    table.update((state) => ({ log: appendLog(state.log, state.log.length, [rewound(0, 7, 'Mana')]) }))
    expect(note()).toHaveTextContent('could not be played: not enough mana')
    expect(table.container.querySelector('[data-slot="nothing-to-do-region"]')).toContainElement(note())
    table.update(() => ({ pending: { id: 1, decision: decisionFor('Priority') } }))
    expect(note()).not.toBeNull()
    table.update(() => ({ pending: null }))
    expect(note()).toBeNull()
  })
})
