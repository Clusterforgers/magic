// Goal: each question the lobby asks says in words what will happen and to whom, names the player and the seat number (counted from 1) for a removal, gives the safe answer and the doing answer different labels, and every notice that follows is a whole sentence.
import { describe, expect, it } from 'vitest'
import { CLOSED_NOTICE, LEFT_NOTICE, questionWords, removedNotice, STARTED_NOTICE } from './lobbyQuestions'
import type { Question } from './lobbyQuestions'

const QUESTIONS: Question[] = [
  { kind: 'close' },
  { kind: 'leave' },
  { kind: 'remove', seat: 1, name: 'Bo' },
  { kind: 'remove', seat: 11, name: 'Wolfeschlegelsteinhausenbergerdo' },
]

describe('questionWords', () => {
  it.each(QUESTIONS)('has a title that asks, a description and two different answers for %j', (question) => {
    const words = questionWords(question)
    expect(words.title).toMatch(/\?$/)
    expect(words.description).toMatch(/\.$/)
    expect(words.confirm.length).toBeGreaterThan(3)
    expect(words.cancel.length).toBeGreaterThan(3)
    expect(words.confirm).not.toBe(words.cancel)
  })

  it('says a close sends everyone back to Play and ends the link and the code', () => {
    const { description, confirm } = questionWords({ kind: 'close' })
    expect(description).toContain('Everyone in the lobby is sent back to Play.')
    expect(description).toContain('invite link and the code stop working')
    expect(confirm).toBe('Close game')
  })

  it('says a leave gives the seat to someone else and goes to Play', () => {
    const { description, confirm } = questionWords({ kind: 'leave' })
    expect(description).toContain('Your seat opens for someone else and you go back to Play.')
    expect(confirm).toBe('Leave game')
  })

  it('names the player and counts the seat from 1 for a removal', () => {
    const words = questionWords({ kind: 'remove', seat: 1, name: 'Bo' })
    expect(words.title).toBe('Remove Bo?')
    expect(words.description).toBe(
      'Bo is taken out of seat 2 and sent back to Play. The seat opens for someone else to take.',
    )
    expect(words.confirm).toBe('Remove Bo')
    expect(words.cancel).toBe('Keep Bo')
  })

  it('keeps the whole of a long name', () => {
    const name = 'Wolfeschlegelsteinhausenbergerdo'
    const words = questionWords({ kind: 'remove', seat: 11, name })
    expect(words.title).toContain(name)
    expect(words.description).toContain('seat 12')
  })
})

describe('the notices', () => {
  it.each([CLOSED_NOTICE, LEFT_NOTICE, STARTED_NOTICE, removedNotice('Bo', 1)])(
    'is a whole sentence: %s',
    (notice) => {
      expect(notice).toMatch(/^[A-Z].*\.$/)
    },
  )

  it('says who was removed and that their seat is open', () => {
    expect(removedNotice('Bo', 1)).toBe('Bo was removed. Seat 2 is open.')
  })
})
