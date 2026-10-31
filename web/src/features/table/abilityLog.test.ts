// Goal: the log names an ability that resolves after the card it comes from ("An ability of Gravedigger begins to resolve"), from the events that put it on the stack or from the view's own record, whatever the outcome of the resolution, and never confuses it with the card itself or an unknown object.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, EventView } from '@/protocol/generated'
import { pub } from '@/test-utils/turnEvents'
import { abilitySources } from './eventContext'
import { describeEvent } from './eventText'
import type { EventNames } from './eventText'
import { foldLog } from './logFold'

const CARDS: Record<number, string> = { 7: 'Gravedigger', 4: 'Lightning Bolt', 3: 'Hill Giant' }
const names: EventNames = {
  seat: (seat) => ['Ana', 'Bo'][seat] ?? `Seat ${seat + 1}`,
  card: (id) => CARDS[id] ?? 'a card',
}
const ABILITY = 90
const source = { id: 7, counter: 0 }

const put: Event = {
  AbilityPutOnStack: { object: ABILITY, source, snapshot: {} as never, ability: 0, controller: 0 },
}
const copied: Event = { AbilityCopied: { object: 91, of: { id: ABILITY, counter: 0 }, controller: 1 } }
const began = (object: number): Event => ({ ResolutionBegan: { object } })
const ended = (object: number, outcome: 'Resolved' | 'Fizzled' | 'Lapsed' | 'Countered'): Event => ({
  ResolutionEnded: { object, outcome },
})

const fold = (events: Event[], given: EventNames = names): string[] => {
  const log: LogEntry[] = events.map((event, seq) => ({ seq, event: pub(event) }))
  return foldLog(log, given, { seats: [0, 1] }).map((line) => line.text)
}

describe('an ability that resolves, known from the events that put it on the stack', () => {
  it.each<[string, Event[], string[]]>([
    [
      'resolved',
      [put, began(ABILITY), ended(ABILITY, 'Resolved')],
      [
        'An ability of Gravedigger goes on the stack',
        'An ability of Gravedigger begins to resolve',
        'An ability of Gravedigger finishes resolving',
      ],
    ],
    [
      'fizzled',
      [put, began(ABILITY), ended(ABILITY, 'Fizzled')],
      [
        'An ability of Gravedigger goes on the stack',
        'An ability of Gravedigger begins to resolve',
        'An ability of Gravedigger does nothing: its targets are gone',
      ],
    ],
    [
      'lapsed',
      [put, began(ABILITY), ended(ABILITY, 'Lapsed')],
      [
        'An ability of Gravedigger goes on the stack',
        'An ability of Gravedigger begins to resolve',
        'An ability of Gravedigger does nothing: its condition is no longer true',
      ],
    ],
    [
      'countered by a spell',
      [put, began(4), ended(ABILITY, 'Countered'), ended(4, 'Resolved')],
      [
        'An ability of Gravedigger goes on the stack',
        'Lightning Bolt begins to resolve',
        'Lightning Bolt counters an ability of Gravedigger',
        'Lightning Bolt finishes resolving',
      ],
    ],
    [
      'countered with nothing resolving',
      [put, ended(ABILITY, 'Countered')],
      ['An ability of Gravedigger goes on the stack', 'An ability of Gravedigger is countered'],
    ],
    [
      'a copy of the ability',
      [put, copied, began(91), ended(91, 'Resolved')],
      [
        'An ability of Gravedigger goes on the stack',
        'Bo copies the ability of Gravedigger',
        'An ability of Gravedigger begins to resolve',
        'An ability of Gravedigger finishes resolving',
      ],
    ],
  ])('says it when %s', (_what, events, expected) => {
    expect(fold(events)).toEqual(expected)
  })

  it('names the ability as the one that moves a card while it resolves', () => {
    const bounce: Event = { ZoneChanged: { object: 3, from: 'Battlefield', to: { Hand: 1 }, cause: 'Moved' } }
    const exile: Event = { ZoneChanged: { object: 3, from: 'Battlefield', to: 'Exile', cause: 'Exiled' } }
    expect(fold([put, began(ABILITY), bounce, exile, ended(ABILITY, 'Resolved')]).slice(2, 4)).toEqual([
      "An ability of Gravedigger returns Hill Giant to Bo's hand",
      'Hill Giant is exiled by an ability of Gravedigger',
    ])
  })

  it('keeps the card name for a spell, which no ability put on the stack', () => {
    expect(fold([put, began(4), ended(4, 'Resolved')]).slice(1)).toEqual([
      'Lightning Bolt begins to resolve',
      'Lightning Bolt finishes resolving',
    ])
  })

  it('keeps "a card" for an object nothing says is an ability', () => {
    expect(fold([began(55), ended(55, 'Resolved')])).toEqual([
      'a card begins to resolve',
      'a card finishes resolving',
    ])
  })
})

describe('an ability the view still holds', () => {
  const given: EventNames = { ...names, ability: (id) => (id === ABILITY ? 7 : undefined) }

  it('is named after its source though no event put it on the stack in this window', () => {
    expect(fold([began(ABILITY), ended(ABILITY, 'Resolved')], given)).toEqual([
      'An ability of Gravedigger begins to resolve',
      'An ability of Gravedigger finishes resolving',
    ])
  })

  it('is read in a line without the folded log, from the names alone', () => {
    const line = describeEvent(pub(began(ABILITY)), given)
    expect(line).toEqual({ text: 'An ability of Gravedigger begins to resolve', minor: true })
  })
})

describe('abilitySources', () => {
  const views = (events: Event[]): EventView[] => events.map(pub)

  it('maps every ability object to its card, a copy to the card of the original', () => {
    expect([...abilitySources(views([put, copied]))]).toEqual([
      [ABILITY, 7],
      [91, 7],
    ])
  })

  it('maps a copy of an ability it has not seen to the copied object itself', () => {
    expect([...abilitySources(views([copied]))]).toEqual([[91, ABILITY]])
  })

  it('reads only public events and nothing else', () => {
    expect(abilitySources([{ Hidden: { CardDrawn: { player: 0 } } }, ...views([began(ABILITY)])]).size).toBe(
      0,
    )
  })
})
