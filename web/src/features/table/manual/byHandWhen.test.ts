// Goal: the log line of a cast card with rules left to the table says when each kind of clause applies (an effect of a spell as it resolves, a triggered clause whenever it triggers, any other clause of a permanent while it is on the battlefield), mixes the kinds in the order they are first met, and keeps the plain line once the card resolves or enters.
import { describe, expect, it } from 'vitest'
import type { EventView } from '@/protocol/generated'
import { pub } from '@/test-utils/turnEvents'
import type { EventNames } from '../eventText'
import { applyWhen, byHandLine } from './manualLog'

describe('when a clause applies', () => {
  it.each<[string, string[], boolean, string]>([
    ['an effect of a spell', ['Draw two cards.'], false, 'when it resolves'],
    ['several effects of a spell', ['Draw two cards.', 'Gain 3 life.'], false, 'when it resolves'],
    ['a trigger of a spell', ['Whenever a creature dies, draw a card.'], false, 'whenever it triggers'],
    ['a trigger of a permanent', ['When it enters, scry 2.'], true, 'whenever it triggers'],
    ['a trigger at a step', ['At the beginning of your upkeep, flip a coin.'], true, 'whenever it triggers'],
    ['a trigger at the end of a turn', ['At the end of turn, discard a card.'], true, 'whenever it triggers'],
    [
      'a trigger with an ability word',
      ['Landfall — Whenever a land enters, draw.'],
      true,
      'whenever it triggers',
    ],
    ['a static clause', ['Creatures you control have haste.'], true, 'while it is on the battlefield'],
    ['an activated clause', ['{T}: Flip a coin.'], true, 'while it is on the battlefield'],
    [
      'a static clause and a trigger',
      ['Creatures you control have haste.', 'Whenever you gain life, draw a card.'],
      true,
      'while it is on the battlefield and whenever it triggers',
    ],
    [
      'two triggers and a static clause',
      ['Whenever you attack, scry 1.', 'Hexproof from red.', 'When it dies, draw.'],
      true,
      'whenever it triggers and while it is on the battlefield',
    ],
    [
      'a clause with a trigger word not at its start',
      ['Draw a card whenever you want.'],
      false,
      'when it resolves',
    ],
    ['a clause with leading spaces', ['  Whenever you win a flip, draw.'], true, 'whenever it triggers'],
  ])('says %s', (_what, clauses, permanent, words) => {
    expect(applyWhen(clauses, permanent)).toBe(words)
  })
})

describe('the by-hand line of a cast card', () => {
  const CLAUSES: Record<number, string[]> = {
    1: ['Draw two cards.'],
    2: ['Whenever you gain life, draw a card.'],
    3: ['Creatures you control have haste.'],
  }
  const names: EventNames = {
    seat: () => 'Ana',
    card: (id) => `Card ${id}`,
    clauses: (id) => CLAUSES[id] ?? [],
    permanent: (id) => id !== 1,
  }
  const cast = (object: number): EventView => pub({ SpellCast: { player: 0, object } })
  const resolved = (object: number): EventView => pub({ SpellResolved: { object } })

  it.each<[string, EventView, string]>([
    ['a spell effect', cast(1), 'Card 1 has rules to apply by hand when it resolves: “Draw two cards.”'],
    [
      'a trigger of a permanent spell',
      cast(2),
      'Card 2 has rules to apply by hand whenever it triggers: “Whenever you gain life, draw a card.”',
    ],
    [
      'a static clause of a permanent spell',
      cast(3),
      'Card 3 has rules to apply by hand while it is on the battlefield: “Creatures you control have haste.”',
    ],
    ['a spell effect as it resolves', resolved(1), 'Card 1 has rules to apply by hand: “Draw two cards.”'],
    [
      'a trigger as its permanent resolves',
      resolved(2),
      'Card 2 has rules to apply by hand: “Whenever you gain life, draw a card.”',
    ],
  ])('reads %s', (_what, event, line) => {
    expect(byHandLine(event, names)).toBe(line)
  })

  it('reads a card as a spell when the viewer is not told its type', () => {
    const unknown: EventNames = { ...names, permanent: undefined }
    expect(byHandLine(cast(3), unknown)).toBe(
      'Card 3 has rules to apply by hand when it resolves: “Creatures you control have haste.”',
    )
    expect(byHandLine(cast(2), unknown)).toContain('whenever it triggers')
  })

  it('adds nothing for a card without clauses', () => {
    expect(byHandLine(cast(9), names)).toBeNull()
  })
})
