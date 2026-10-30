// Goal: the line under a row or a menu says in plain words what became of a manual action (sending, done, refused with the server's reason, not confirmed, not sent), and a session's answer maps to exactly one of them.
import { describe, expect, it } from 'vitest'
import { feedbackOf, feedbackWords } from './feedback'
import type { Feedback } from './feedback'

describe('the words of a feedback line', () => {
  it.each<[Feedback, string]>([
    [{ state: 'sending', what: 'Lose 5 life' }, 'Sending: Lose 5 life…'],
    [{ state: 'done', what: 'Lose 5 life' }, 'Done: Lose 5 life.'],
    [{ state: 'refused', reason: 'you are not in the game' }, 'Refused: you are not in the game.'],
    [
      { state: 'refused', reason: '  Grizzly Bears is already tapped.  ' },
      'Refused: Grizzly Bears is already tapped.',
    ],
    [{ state: 'refused', reason: 'Is that allowed?' }, 'Refused: Is that allowed?'],
    [
      { state: 'lost', reason: 'the connection was interrupted' },
      'Not confirmed: the connection was interrupted.',
    ],
    [{ state: 'offline' }, 'Not sent: the connection is not open.'],
  ])('%j reads "%s"', (feedback, words) => {
    expect(feedbackWords(feedback)).toBe(words)
  })
})

describe('the feedback a session answer stands for', () => {
  it.each([
    [{ ok: true }, { state: 'done', what: 'Draw 2 cards' }],
    [
      { ok: false, reason: 'no' },
      { state: 'refused', reason: 'no' },
    ],
    [
      { ok: false, reason: 'gone', lost: true },
      { state: 'lost', reason: 'gone' },
    ],
  ] as const)('%j is %j', (result, feedback) => {
    expect(feedbackOf(result, 'Draw 2 cards')).toEqual(feedback)
  })
})
