// Goal: the log keeps step lines out of the way of plays: each turn has one head, a step is named only above the plays that happened in it, a step in which nothing was read (untaps, passes, mana, no attackers) is no line at all, and every play, trigger, damage and life line stays whole and in order, for 2 and 4 seats.
import { describe, expect, it } from 'vitest'
import type { LogEntry } from '@/play/store'
import type { EventView, PlayerId, Step } from '@/protocol/generated'
import { begin, pass, pub, round, turnBegan } from '@/test-utils/turnEvents'
import { foldLog, withoutEmptySteps } from './logFold'
import type { FoldedLine, LineKind } from './logFold'

const NAMES = ['Alice', 'Bo', 'Cy', 'Dee']
const names = { seat: (seat: PlayerId) => NAMES[seat] ?? `Seat ${seat}`, card: (id: number) => `Card ${id}` }

const cast = (player: PlayerId, object: number): EventView => pub({ SpellCast: { player, object } })
const resolved = (object: number): EventView => pub({ SpellResolved: { object } })
const life = (player: PlayerId, amount: number): EventView => pub({ LifeLost: { player, amount } })
const land = (player: PlayerId, object: number): EventView => pub({ LandPlayed: { player, object } })
const noAttack = (player: PlayerId): EventView => pub({ AttackersDeclared: { player, attackers: [] } })
const noBlock = (player: PlayerId): EventView => pub({ BlockersDeclared: { player, blocks: [] } })
const nothing = (player: PlayerId): EventView => pub({ NothingToDo: { player, kind: 'Attack' } })

const logOf = (events: readonly EventView[]): LogEntry[] => events.map((event, seq) => ({ seq, event }))
const lines = (events: readonly EventView[], count = 2): FoldedLine[] =>
  foldLog(logOf(events), names, { seats: Array.from({ length: count }, (_, seat) => seat) })
/** "kind:text" of each line, to read a log as a player does. */
const shape = (events: readonly EventView[], count = 2): string[] =>
  lines(events, count).map((line) => `${line.kind}:${line.text}`)

describe.each([2, 4])('the log of a turn at a table of %i', (count) => {
  /** A step with nothing to read in it: it begins and every seat passes. */
  const quiet = (step: Step): EventView[] => [begin(step), ...round(0, count)]

  it('names a step only above the plays that happened in it, under one head for the turn', () => {
    const events = [
      turnBegan(5, 0),
      ...quiet('Untap'),
      ...quiet('Upkeep'),
      begin('Draw'),
      pub({ CardDrawn: { player: 0, object: 3 } }),
      begin('Main1'),
      land(0, 4),
      cast(0, 7),
      ...round(0, count),
      resolved(7),
      ...quiet('BeginCombat'),
      begin('DeclareAttackers'),
      nothing(0),
      noAttack(0),
      ...quiet('EndCombat'),
      ...quiet('Main2'),
      ...quiet('End'),
      ...quiet('Cleanup'),
    ]
    expect(shape(events, count)).toEqual([
      "turn:Turn 5 begins: Alice's turn",
      'step:Draw',
      'play:Alice draws Card 3',
      'step:Precombat main',
      'play:Alice plays Card 4',
      'play:Alice casts Card 7',
      'play:All pass',
      'play:Card 7 resolves',
      'play:Alice has no possible attackers',
      'play:Alice declares no attackers',
    ])
  })

  it('shows the head of every turn and each step of a turn in which combat happened', () => {
    const events = [
      turnBegan(1, 0),
      begin('DeclareAttackers'),
      pub({ AttackersDeclared: { player: 0, attackers: [[9, { Player: 1 }]] } }),
      begin('CombatDamage'),
      life(1, 3),
      turnBegan(2, 1),
      begin('Main1'),
      land(1, 5),
    ]
    expect(lines(events, count).map((line) => line.kind)).toEqual([
      'turn',
      'step',
      'play',
      'step',
      'play',
      'turn',
      'step',
      'play',
    ])
    expect(lines(events, count).filter((line) => line.kind === 'turn')).toHaveLength(2)
  })
})

describe('withoutEmptySteps', () => {
  const line = (kind: LineKind, text: string, minor = false): FoldedLine => ({
    kind,
    text,
    minor,
    seq: 0,
    passed: [],
    spoken: '',
  })

  it.each<[string, FoldedLine[], string[]]>([
    ['nothing', [], []],
    ['a step with a play after it', [line('step', 'A', true), line('play', 'a play')], ['A', 'a play']],
    ['a step with only minor lines', [line('step', 'A', true), line('play', 'taps', true)], ['taps']],
    ['a step with nothing after it', [line('step', 'A', true)], []],
    ['a step ahead of another step', [line('step', 'A', true), line('step', 'B', true)], []],
    ['a step ahead of a turn head', [line('step', 'A', true), line('turn', 'Turn 2')], ['Turn 2']],
    [
      'a minor line first, then a play',
      [line('step', 'A', true), line('play', 'taps', true), line('play', 'a play')],
      ['A', 'taps', 'a play'],
    ],
    [
      'a play of the next step does not count',
      [line('step', 'A', true), line('play', 'taps', true), line('step', 'B', true), line('play', 'a play')],
      ['taps', 'B', 'a play'],
    ],
    [
      'plays and heads are never left out',
      [line('turn', 'T'), line('play', 'p', true), line('play', 'q')],
      ['T', 'p', 'q'],
    ],
  ])('%s', (_name, input, expected) => {
    expect(withoutEmptySteps(input).map((entry) => entry.text)).toEqual(expected)
  })
})

describe('the lines of play of a log', () => {
  it('keep their words, their order and their weight beside the step lines', () => {
    const events = [
      turnBegan(3, 1),
      begin('Main1'),
      cast(1, 7),
      pass(0),
      pass(1),
      resolved(7),
      life(0, 2),
      begin('End'),
    ]
    const folded = lines(events)
    expect(
      folded.filter((entry) => entry.kind === 'play' && !entry.minor).map((entry) => entry.text),
    ).toEqual(['Bo casts Card 7', 'Card 7 resolves', 'Alice loses 2 life'])
    expect(folded.map((entry) => entry.seq)).toEqual(
      [...folded.map((entry) => entry.seq)].sort((a, b) => a - b),
    )
  })

  it('count as one unread line each, the heads of turns included and the steps not', () => {
    const folded = lines([turnBegan(3, 1), begin('Main1'), noAttack(1), noBlock(0), land(1, 5)])
    expect(folded.filter((entry) => !entry.minor).map((entry) => entry.kind)).toEqual(['turn', 'play'])
  })
})
