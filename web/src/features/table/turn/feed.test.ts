// Goal: the turn feed turns a batch into the cues the table draws (a banner when the turn or the active seat changes, the steps that went by without a window, and at most three ticker lines: the skip note, the viewer's own reason and the newest event worth reading) from the game before it, the game after it and its events alone, explains only the viewer's own skips, and never names a card the viewer alone could see.
import { describe, expect, it } from 'vitest'
import { must } from '@/test-utils/batches'
import {
  begin,
  grant,
  hid,
  momentAt,
  nameOf,
  pass,
  pub,
  round,
  stepRound,
  turnBegan,
} from '@/test-utils/turnEvents'
import type { TurnAt } from '@/test-utils/turnEvents'
import type { EventView, Stage } from '@/protocol/generated'
import { deriveFeed, OWN_REASON, skipNote } from './feed'
import type { Feed, TickerKind } from './feed'

const at = (number: number, active: number, step: TurnAt['step']): TurnAt => ({ number, active, step })

/** Everything that happens between the end of one turn and the first window of the next, with nobody asked. */
const passTurn = (next: number, active: number): EventView[] => [
  ...stepRound('End', 1 - active),
  begin('Cleanup'),
  turnBegan(next, active),
  begin('Untap'),
  ...stepRound('Upkeep', active),
  ...stepRound('Draw', active),
  begin('Main1'),
  grant(active),
]

const lines = (feed: Feed): Array<[TickerKind, string]> => feed.lines.map((line) => [line.kind, line.text])

interface Case {
  name: string
  before: TurnAt
  after: TurnAt
  events: EventView[]
  stage?: Stage
  expected: Partial<Feed>
}

const cases: Case[] = [
  {
    name: 'the turn passes to another seat with Upkeep and Draw going by',
    before: at(5, 0, 'End'),
    after: at(6, 1, 'Main1'),
    events: [
      pass(0),
      begin('Cleanup'),
      turnBegan(6, 1),
      begin('Untap'),
      ...stepRound('Upkeep', 1),
      begin('Draw'),
      hid({ CardDrawn: { player: 1 } }),
      ...round(1),
      begin('Main1'),
      grant(1),
    ],
    expected: {
      banner: { yours: false, who: "Bo's turn", turn: 6 },
      skipped: ['Upkeep', 'Draw'],
      lines: [
        { kind: 'skipped', text: 'Skipped Upkeep, Draw' },
        { kind: 'event', text: 'Bo draws a card' },
      ],
      announcement: "Bo's turn, turn 6. Skipped Upkeep, Draw",
    },
  },
  {
    name: 'the turn comes round to the viewer',
    before: at(6, 1, 'End'),
    after: at(7, 0, 'Main1'),
    events: passTurn(7, 0),
    expected: {
      banner: { yours: true, who: 'Your turn', turn: 7 },
      skipped: ['Upkeep', 'Draw'],
      announcement: 'Your turn, turn 7. Skipped End step, Upkeep, Draw',
    },
  },
  {
    name: 'a step that goes on across batches is not skipped, and nothing changes the banner',
    before: at(5, 1, 'Upkeep'),
    after: at(5, 1, 'Draw'),
    events: [pass(1), begin('Draw'), grant(1)],
    expected: { banner: null, skipped: [], lines: [] },
  },
  {
    name: 'a step without a round of priority (no attackers) is not a skipped window',
    before: at(5, 1, 'Main1'),
    after: at(5, 1, 'EndCombat'),
    events: [
      pass(1),
      ...stepRound('BeginCombat', 1),
      begin('DeclareAttackers'),
      pub({ NothingToDo: { player: 1, kind: 'Attack' } }),
      begin('EndCombat'),
      grant(1),
    ],
    expected: { skipped: ['BeginCombat'], lines: [{ kind: 'skipped', text: 'Skipped Beginning of combat' }] },
  },
  {
    name: 'another seat with the same turn number takes the turn',
    before: at(5, 1, 'Main2'),
    after: at(5, 0, 'Main2'),
    events: [pass(1)],
    expected: { banner: { yours: true, who: 'Your turn', turn: 5 } },
  },
  {
    name: 'the first turn of the game',
    before: at(0, 0, 'Untap'),
    after: at(1, 0, 'Upkeep'),
    stage: 'Turns',
    events: [begin('Untap'), turnBegan(1, 0), begin('Upkeep'), grant(0)],
    expected: { banner: { yours: true, who: 'Your turn', turn: 1 }, skipped: [] },
  },
  {
    name: 'the end of the game',
    before: at(9, 1, 'CombatDamage'),
    after: at(9, 1, 'CombatDamage'),
    stage: 'Finished',
    events: [pub({ GameEnded: { result: { Winner: 0 } } })],
    expected: { banner: null, lines: [{ kind: 'event', text: 'Alice wins the game' }] },
  },
]

describe('the feed of a batch', () => {
  it.each(cases)('$name', ({ before, after, events, stage, expected }) => {
    const feed = deriveFeed(momentAt(before), momentAt(after, stage), events, nameOf)
    expect(feed).toMatchObject(expected)
  })

  it('lists the steps of the turn the batch ends in for the ladder, and all of them in the ticker', () => {
    const feed = deriveFeed(momentAt(at(6, 1, 'End')), momentAt(at(7, 0, 'Main1')), passTurn(7, 0), nameOf)
    expect(feed.skipped).toEqual(['Upkeep', 'Draw'])
    expect(lines(feed)[0]).toEqual(['skipped', 'Skipped End step, Upkeep, Draw'])
  })

  it('says nothing about a batch that moves no step, turn or line worth reading', () => {
    const quiet = [pub({ PermanentTapped: { object: 1 } }), pub({ ManaPoolEmptied: { player: 0 } })]
    const feed = deriveFeed(momentAt(at(5, 1, 'Main1')), momentAt(at(5, 1, 'Main1')), quiet, nameOf)
    expect(feed).toEqual({ banner: null, lines: [], skipped: [], announcement: '' })
  })
})

describe('the ticker lines', () => {
  const card = must(momentAt(at(5, 1, 'Main1')).view.objects.find((object) => object.zone === 'Battlefield'))
  const play = pub({ LandPlayed: { player: 1, object: card.id } })
  const feedOf = (events: EventView[], over: { yielding?: boolean } = {}): Feed =>
    deriveFeed(momentAt(at(5, 1, 'Main1')), momentAt(at(5, 1, 'Main1')), events, nameOf, over)

  it('shows the newest line that is not minor, skipping what the log draws quietly', () => {
    const feed = feedOf([play, pub({ PermanentTapped: { object: card.id } }), pass(1), grant(0)])
    expect(lines(feed)).toEqual([['event', `Bo plays ${card.name}`]])
  })

  it('leaves the start of a turn to the banner instead of repeating it', () => {
    const feed = deriveFeed(
      momentAt(at(5, 1, 'End')),
      momentAt(at(6, 0, 'Upkeep')),
      [begin('Cleanup'), turnBegan(6, 0), begin('Untap'), begin('Upkeep'), grant(0)],
      nameOf,
    )
    expect(feed.banner).not.toBeNull()
    expect(feed.lines).toEqual([])
  })

  it('counts the steps past three instead of listing them', () => {
    expect(skipNote(['Upkeep', 'Draw'])).toBe('Skipped Upkeep, Draw')
    expect(skipNote(['Upkeep', 'Draw', 'Main1'])).toBe('Skipped Upkeep, Draw, Precombat main')
    expect(skipNote(['Upkeep', 'Draw', 'Main1', 'BeginCombat', 'EndCombat'])).toBe(
      'Skipped Upkeep, Draw, Precombat main and 2 more',
    )
    expect(skipNote(['End', 'Upkeep', 'Draw', 'End', 'Upkeep'])).toBe('Skipped End step, Upkeep, Draw')
  })

  it('puts at most three lines in a batch, in the order skip note, reason, newest event', () => {
    const events = [
      begin('Upkeep'),
      ...round(0),
      begin('Main1'),
      ...round(0),
      begin('BeginCombat'),
      ...round(0),
      begin('Main2'),
      play,
    ]
    const feed = deriveFeed(momentAt(at(5, 0, 'End')), momentAt(at(5, 0, 'Main2')), events, nameOf)
    expect(feed.lines.map((line) => line.kind)).toEqual(['skipped', 'reason', 'event'])
    expect(feed.lines).toHaveLength(3)
  })
})

describe('explaining a skip', () => {
  const own = (events: EventView[], yielding = false): Feed =>
    deriveFeed(momentAt(at(5, 0, 'Upkeep')), momentAt(at(5, 0, 'Main2')), events, nameOf, { yielding })
  const mains = [...stepRound('Main1', 0), ...stepRound('BeginCombat', 0), begin('Main2'), grant(0)]

  it("explains the viewer's own main phase that the game passed", () => {
    expect(lines(own(mains))).toEqual([
      ['skipped', 'Skipped Precombat main, Beginning of combat'],
      ['reason', OWN_REASON],
    ])
  })

  it('does not blame a hand for a pass the viewer asked for', () => {
    expect(lines(own(mains, true))).toEqual([['skipped', 'Skipped Precombat main, Beginning of combat']])
  })

  it("never explains another seat's pass, in its main phase or anywhere else", () => {
    const theirs = deriveFeed(
      momentAt(at(5, 1, 'Upkeep')),
      momentAt(at(5, 1, 'Main2')),
      [...stepRound('Main1', 1), ...stepRound('BeginCombat', 1), begin('Main2'), grant(1)],
      nameOf,
    )
    expect(lines(theirs)).toEqual([['skipped', 'Skipped Precombat main, Beginning of combat']])
    for (const line of theirs.lines) {
      expect(line.text).not.toMatch(/auto|no plays|nothing|deciding|thinking|waiting|fast|quick|seconds/i)
    }
  })

  it('does not explain steps that are not a main phase, even the viewer’s own', () => {
    const feed = deriveFeed(momentAt(at(5, 0, 'End')), momentAt(at(6, 1, 'Main1')), passTurn(6, 1), nameOf)
    expect(feed.lines.map((line) => line.kind)).toEqual(['skipped'])
  })
})

describe('what the viewer alone could see', () => {
  const card = must(momentAt(at(5, 0, 'Main1')).view.objects.find((object) => object.zone === 'Battlefield'))
  const draws = (events: EventView[]): string[] =>
    deriveFeed(momentAt(at(5, 0, 'Draw')), momentAt(at(5, 0, 'Draw')), events, nameOf).lines.map(
      (line) => line.text,
    )

  it('words the viewer’s own draw as any seat’s draw, without the card', () => {
    expect(draws([pub({ CardDrawn: { player: 0, object: card.id } })])).toEqual(['Alice draws a card'])
    expect(draws([hid({ CardDrawn: { player: 1 } })])).toEqual(['Bo draws a card'])
  })

  it('gives a move into a hand or a library no line, as it does for every other seat, so no card shows', () => {
    const moved = (to: { Hand: number } | { Library: number }) =>
      draws([pub({ ZoneChanged: { object: card.id, from: 'Battlefield', to, cause: 'Moved' } })])
    expect(moved({ Hand: 0 })).toEqual([])
    expect(moved({ Library: 0 })).toEqual([])
  })

  it('still names a card that moves in the open', () => {
    const events = [
      pub({
        ZoneChanged: { object: card.id, from: 'Battlefield', to: { Graveyard: 0 }, cause: 'Destroyed' },
      }),
    ]
    expect(draws(events)).toEqual([`${card.name} is destroyed`])
  })
})
