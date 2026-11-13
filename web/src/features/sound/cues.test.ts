// Goal: each change of the play store calls for at most one sound cue, the strongest that applies (a win of the viewer or its team, the viewer's own turn or their team's shared turn, a decision after none), and a first view, a spectator, another seat's turn or decision, a draw and a loss call for none.
import { describe, expect, it } from 'vitest'
import { initialPlayState } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { Decision, GameResult } from '@/protocol/generated'
import { decisionFor } from '@/test-utils/decisions'
import { FOE, ME, miniView } from '@/test-utils/miniTable'
import { soundCueBetween } from './cues'
import type { SoundCue } from './cues'

interface At {
  turn?: number
  active?: number
  pending?: Decision | null
  result?: GameResult | null
  spectator?: boolean
  noView?: boolean
  teams?: number[]
  giant?: boolean
}

function at(over: At = {}): PlayState {
  const view = miniView()
  const turn = { ...view.turn, number: over.turn ?? 3, active: over.active ?? FOE }
  return {
    ...initialPlayState(),
    seat: ME,
    view:
      over.noView === true
        ? null
        : {
            ...view,
            turn,
            spectator: over.spectator,
            house_rules:
              over.teams === undefined
                ? undefined
                : { teams: { mode: over.giant === true ? 'two_headed_giant' : 'teams', seats: over.teams } },
          },
    pending: over.pending === undefined || over.pending === null ? null : { id: 7, decision: over.pending },
    result: over.result ?? null,
  }
}

const mine = decisionFor('Priority', ME)
const theirs = decisionFor('Priority', FOE)

const cases: [string, At, At, SoundCue | null][] = [
  ['nothing changes', {}, {}, null],
  ['the viewer wins', {}, { result: { Winner: ME } }, 'won'],
  ['the viewer wins on their own new turn', {}, { turn: 4, active: ME, result: { Winner: ME } }, 'won'],
  ['another seat wins', {}, { result: { Winner: FOE } }, null],
  ['the viewer’s team wins', {}, { teams: [0, 1], result: { Team: 0 } }, 'won'],
  ['another team wins', {}, { teams: [0, 1], result: { Team: 1 } }, null],
  ['a draw', {}, { result: 'Draw' }, null],
  ['the viewer’s turn begins', {}, { turn: 4, active: ME }, 'turn'],
  ['the viewer’s turn begins with a decision', {}, { turn: 4, active: ME, pending: mine }, 'turn'],
  ['another seat’s turn begins', { active: ME }, { turn: 4, active: FOE }, null],
  [
    'the team’s shared turn begins in Two-Headed Giant',
    {},
    { turn: 4, active: FOE, teams: [0, 0], giant: true },
    'turn',
  ],
  ['a teammate’s own turn begins in Teams', {}, { turn: 4, active: FOE, teams: [0, 0] }, null],
  ['a decision arrives while none was pending', {}, { pending: mine }, 'decision'],
  ['a decision follows a decision', { pending: mine }, { pending: mine }, null],
  ['another seat’s decision', {}, { pending: theirs }, null],
  ['the first view arrives with a decision', { noView: true }, { pending: mine, turn: 4, active: ME }, null],
  ['a spectator’s table', {}, { spectator: true, turn: 4, active: ME }, null],
]

describe('soundCueBetween', () => {
  it.each(cases)('%s', (_, before, after, cue) => {
    expect(soundCueBetween(at(before), at(after))).toBe(cue)
  })

  it('plays nothing for a client with no seat', () => {
    expect(soundCueBetween(at(), { ...at({ result: { Winner: ME } }), seat: null })).toBeNull()
  })
})
