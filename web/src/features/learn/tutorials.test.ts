// Goal: a tab keeps one practice game: each lesson it starts replaces the game it started before, leaving a lesson ends its game and forgets it, and a store that refuses or holds garbage leaves the server to end the game after its grace.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { leaveLesson, PRACTICE_KEY, practiceSeat, startLesson, tutorialOf } from './tutorials'

const seat = (n: number) => ({ game: `g-${n}`, seat: 0, token: `t-${n}` })

function api() {
  let n = 0
  return {
    startTutorial: vi.fn(() => Promise.resolve(seat((n += 1)))),
    endTutorial: vi.fn(() => Promise.resolve(true)),
  }
}

describe('practice games of a tab', () => {
  beforeEach(() => window.sessionStorage.clear())

  it('starts the first lesson without a game to replace, and each later one replacing the one before', async () => {
    const client = api()
    await startLesson(client, 'seat')
    expect(client.startTutorial).toHaveBeenLastCalledWith('seat')
    expect(practiceSeat()).toEqual({ game: 'g-1', token: 't-1' })
    await startLesson(client, 'seat')
    expect(client.startTutorial).toHaveBeenLastCalledWith('seat', { game: 'g-1', token: 't-1' })
    await startLesson(client, 'mana')
    expect(client.startTutorial).toHaveBeenLastCalledWith('mana', { game: 'g-2', token: 't-2' })
    expect(practiceSeat()).toEqual({ game: 'g-3', token: 't-3' })
    expect(tutorialOf('g-3')).toBe('mana')
  })

  it.each([
    ['the game it plays', 'g-1', undefined],
    ['an older game', 'g-0', { game: 'g-1', token: 't-1' }],
  ])('ends %s when the learner leaves, forgetting only the game it plays', async (_, game, kept) => {
    const client = api()
    await startLesson(client, 'combat')
    await leaveLesson(client, { game, token: 'x' })
    expect(client.endTutorial).toHaveBeenCalledWith({ game, token: 'x' })
    expect(practiceSeat()).toEqual(kept)
  })

  it.each([['not json'], ['{"game":1,"token":"t"}'], ['null'], ['"g-1"']])(
    'reads a stored %s as no game, so the next lesson replaces nothing',
    async (stored) => {
      window.sessionStorage.setItem(PRACTICE_KEY, stored)
      const client = api()
      await startLesson(client, 'stack')
      expect(client.startTutorial).toHaveBeenLastCalledWith('stack')
    },
  )
})
