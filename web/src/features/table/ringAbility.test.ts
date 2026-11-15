// Goal: an ability the Ring gives its bearer is the Ring's in the log and on the stack ("The Ring's ability triggers (Nazgûl, Ana's Ring-bearer): draw a card, then discard a card"), at every level and for the delayed trigger a level sets up, from the events or from the view's own record, while an ability the bearer prints keeps the bearer's name and the card the loot draws is in the log between the resolution and the end of it.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { Event, ObjectView, TriggerEntry } from '@/protocol/generated'
import { pub } from '@/test-utils/turnEvents'
import { describeEvent } from './eventText'
import type { EventNames } from './eventText'
import { granterAbility, granterClause, granterOfOrigin } from './granter'
import { foldLog } from './logFold'
import { triggerLabels } from './prompt/orderText'
import { stackSummary } from './stackPreview'
import type { TargetNamer } from './stackText'

const CARDS: Record<number, string> = { 7: 'Nazgûl', 4: 'Lightning Bolt', 30: 'Forest' }
const names: EventNames = {
  seat: (seat) => ['Ana', 'Bo'][seat] ?? `Seat ${seat + 1}`,
  card: (id) => CARDS[id] ?? 'a card',
}
const ABILITY = 90
const bearer = { id: 7, counter: 0 }

const ring = (level: number) => ({ Granted: { Ring: { player: 0, level } } }) as const
const triggered = (origin: unknown, delayed?: number): Event =>
  ({
    AbilityTriggered: {
      source: bearer,
      ability: 0,
      origin,
      controller: 0,
      cause: {} as never,
      snapshot: {} as never,
      ...(delayed === undefined ? {} : { delayed }),
    },
  }) as Event
const put = (origin: unknown): Event =>
  ({
    AbilityPutOnStack: {
      object: ABILITY,
      source: bearer,
      snapshot: {} as never,
      ability: 0,
      origin,
      controller: 0,
      cause: {} as never,
    },
  }) as Event

const fold = (events: Event[], given: EventNames = names): string[] => {
  const log: LogEntry[] = events.map((event, seq) => ({ seq, event: pub(event) }))
  return foldLog(log, given, { seats: [0, 1] }).map((line) => line.text)
}

describe('the Ring names the abilities it gives', () => {
  it.each([
    [2, "The Ring's ability triggers (Nazgûl, Ana's Ring-bearer): draw a card, then discard a card"],
    [
      3,
      "The Ring's ability triggers (Nazgûl, Ana's Ring-bearer): that creature's controller sacrifices it at end of combat",
    ],
    [4, "The Ring's ability triggers (Nazgûl, Ana's Ring-bearer): each opponent loses 3 life"],
  ])('level %i: the trigger names the Ring, the bearer and what it does', (level, text) => {
    expect(describeEvent(pub(triggered(ring(level))), names).text).toBe(text)
  })

  it('names the delayed trigger a level sets up without a clause that describes something else', () => {
    expect(describeEvent(pub(triggered(ring(3), 5)), names).text).toBe(
      "The Ring's delayed ability triggers (Nazgûl, Ana's Ring-bearer)",
    )
  })

  it('names the seat that has the Ring, not the viewer', () => {
    const line = describeEvent(
      pub({
        AbilityTriggered: {
          source: bearer,
          ability: 0,
          origin: { Granted: { Ring: { player: 1, level: 2 } } },
          controller: 1,
          cause: {} as never,
          snapshot: {} as never,
        },
      }),
      names,
    ).text
    expect(line).toBe(
      "The Ring's ability triggers (Nazgûl, Bo's Ring-bearer): draw a card, then discard a card",
    )
  })

  it('keeps the name of the card for an ability the card prints or any other grant gives', () => {
    for (const origin of [undefined, 'Printed', { Granted: { Effect: 12 } }, { PrintedKeyword: 0 }]) {
      expect(describeEvent(pub(triggered(origin)), names).text).toBe('An ability of Nazgûl triggers')
    }
  })

  it('tells the loot from the trigger to the end of its resolution, the draw between', () => {
    const lines = fold([
      triggered(ring(2)),
      put(ring(2)),
      { ResolutionBegan: { object: ABILITY } },
      { CardDrawn: { player: 0, object: 30 } },
      { ResolutionEnded: { object: ABILITY, outcome: 'Resolved' } },
    ])
    expect(lines).toEqual([
      "The Ring's ability triggers (Nazgûl, Ana's Ring-bearer): draw a card, then discard a card",
      "The Ring's ability goes on the stack",
      "The Ring's ability begins to resolve",
      'Ana draws Forest',
      "The Ring's ability finishes resolving",
    ])
  })

  it('names the Ring after it left the stack, from the view of an object that is still there', () => {
    const lines = fold(
      [
        { ResolutionBegan: { object: ABILITY } },
        { ResolutionEnded: { object: ABILITY, outcome: 'Fizzled' } },
      ],
      {
        ...names,
        granter: (id) => (id === ABILITY ? { Ring: { level: 2 } } : undefined),
      },
    )
    expect(lines).toEqual([
      "The Ring's ability begins to resolve",
      "The Ring's ability does nothing: its targets are gone",
    ])
  })

  it('names what a Ring ability does to a card as the Ring does it', () => {
    const lines = fold([
      put(ring(4)),
      { ResolutionBegan: { object: ABILITY } },
      {
        ZoneChanged: {
          object: 4,
          from: 'Battlefield',
          to: { Graveyard: 1 },
          cause: 'Destroyed',
        } as never,
      },
    ])
    expect(lines[2]).toBe("Lightning Bolt is destroyed by the Ring's ability")
  })

  it('names the Ring when a trigger it gave is removed or its delayed trigger is set up', () => {
    const removed = {
      TriggerRemoved: { source: bearer, ability: 0, origin: ring(2), cause: {} as never },
    } as Event
    expect(describeEvent(pub(removed), names).text).toBe('A triggered ability of The Ring is removed')
    const created = { DelayedTriggerCreated: { source: bearer, origin: ring(3) } } as Event
    expect(describeEvent(pub(created), names).text).toBe('A delayed ability of The Ring is set up')
    const plain = { DelayedTriggerCreated: { source: bearer } } as Event
    expect(describeEvent(pub(plain), names).text).toBe('A delayed ability of Nazgûl is set up')
  })
})

describe('the words of the Ring', () => {
  it('reads the Ring from an origin and from nothing else', () => {
    expect(granterOfOrigin(ring(3))).toEqual({ Ring: { level: 3 } })
    expect(granterOfOrigin(undefined)).toBeUndefined()
    expect(granterOfOrigin('Printed')).toBeUndefined()
    expect(granterOfOrigin({ Granted: { Effect: 3 } })).toBeUndefined()
    expect(granterOfOrigin({ GrantedKeyword: { Ring: { player: 0, level: 2 } } })).toBeUndefined()
  })

  it('says a clause for each level that gives an ability and none for any other', () => {
    expect([1, 2, 3, 4, 5].map((level) => granterClause({ Ring: { level } }) !== undefined)).toEqual([
      false,
      true,
      true,
      true,
      false,
    ])
  })

  it('writes "The" at the start of a sentence and "the" inside one', () => {
    expect(granterAbility({ Ring: { level: 2 } }, true)).toBe("The Ring's ability")
    expect(granterAbility({ Ring: { level: 2 } }, false)).toBe("the Ring's ability")
  })
})

describe('the stack entry of the Ring', () => {
  const seat = (id: number): string => names.seat(id)
  const targets: TargetNamer = { seat, card: () => 'Hill Giant' }
  const entry = (over: Partial<ObjectView>): ObjectView =>
    ({
      id: ABILITY,
      owner: 0,
      controller: 0,
      zone: 'Stack',
      name: 'The Ring',
      kind: 'Ability',
      source: 7,
      granter: { Ring: { level: 2 } },
      rules_text: 'Whenever your Ring-bearer attacks, draw a card, then discard a card.',
      ...over,
    }) as ObjectView
  const nazgul = { id: 7, name: 'Nazgûl', zone: 'Battlefield' } as ObjectView

  it('is the Ring’s triggered ability with the Ring-bearer named beside it', () => {
    const summary = stackSummary(entry({}), nazgul, undefined, seat, targets)
    expect(summary.heading).toBe(
      "Triggered ability of The Ring (Nazgûl, Ana's Ring-bearer), controlled by Ana",
    )
    expect(summary.rules).toEqual(['Whenever your Ring-bearer attacks, draw a card, then discard a card.'])
  })

  it('leaves the bearer out while the view does not hold it', () => {
    expect(stackSummary(entry({}), undefined, undefined, seat, targets).heading).toBe(
      'Triggered ability of The Ring, controlled by Ana',
    )
  })

  it('leaves an ability the bearer prints as the bearer’s own', () => {
    const own = entry({
      name: 'Nazgûl',
      granter: undefined,
      rules_text: 'Whenever Nazgûl attacks, you gain 1 life.',
    })
    expect(stackSummary(own, nazgul, undefined, seat, targets).heading).toBe(
      'Triggered ability of Nazgûl, controlled by Ana',
    )
  })
})

describe('the order of triggers', () => {
  it('labels the Ring’s trigger by the Ring and a trigger the bearer prints by the bearer', () => {
    const entries = [
      { source: bearer, ability: 0, when: 'Whenever it attacks', origin: ring(2) },
      { source: bearer, ability: 0, when: 'Whenever it attacks' },
    ] as TriggerEntry[]
    expect(triggerLabels(entries, ['Nazgûl', 'Nazgûl'])).toEqual([
      'The Ring: Whenever it attacks',
      'Nazgûl: Whenever it attacks',
    ])
  })
})
