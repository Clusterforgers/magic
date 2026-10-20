// Goal: the form model starts from sensible defaults, stays between 2 and 12 seats, judges names exactly as the server does, and always builds a request body of the documented shape.
import { describe, expect, it } from 'vitest'
import { MAX_NAME_CHARS, MAX_SEATS, MIN_SEATS } from '@/lib/limits'
import { DECKS } from '@/test-utils/decks'
import {
  addSeat,
  chooseLinks,
  chooseVisibility,
  defaultDraft,
  editSeat,
  nameProblem,
  problemCount,
  problemsOf,
  removeSeat,
  resizeSeats,
  toRequest,
} from './form'
import type { Draft } from './form'

const grown = (to: number, decks = DECKS): Draft => {
  let draft = defaultDraft(decks)
  while (draft.seats.length < to) draft = addSeat(draft, decks)
  return draft
}

const seatCounts = Array.from({ length: MAX_SEATS - MIN_SEATS + 1 }, (_, i) => MIN_SEATS + i)

describe('the starting form', () => {
  it('has two seats named Player 1 and Player 2 on different decks', () => {
    const { seats } = defaultDraft(DECKS)
    expect(seats.map((seat) => seat.name)).toEqual(['Player 1', 'Player 2'])
    expect(seats.map((seat) => seat.deck)).toEqual(['mono-green', 'mono-red'])
  })

  it('shares the only deck when the server has just one', () => {
    const { seats } = defaultDraft(DECKS.slice(0, 1))
    expect(seats.map((seat) => seat.deck)).toEqual(['mono-green', 'mono-green'])
  })

  it('leaves the deck empty, and so invalid, when the server has none', () => {
    const draft = defaultDraft([])
    expect(draft.seats.map((seat) => seat.deck)).toEqual(['', ''])
    // Only the creator's seat is filled in; the open seat is judged once it gets a link of its own.
    expect(problemCount(problemsOf(draft, []))).toBe(1)
    expect(problemCount(problemsOf(chooseLinks(draft, true), []))).toBe(2)
  })

  it('lists the game and gives every seat a link only when asked', () => {
    const draft = defaultDraft(DECKS)
    expect(draft.visibility).toBe('public')
    expect(draft.links).toBe(false)
  })

  it('has no problems', () => {
    expect(problemCount(problemsOf(defaultDraft(DECKS), DECKS))).toBe(0)
  })

  it('names the first seat after the signed-in creator and leaves the rest alone', () => {
    const { seats } = defaultDraft(DECKS, 'Ada_L')
    expect(seats.map((seat) => seat.name)).toEqual(['Ada_L', 'Player 2'])
    expect(seats.map((seat) => seat.deck)).toEqual(['mono-green', 'mono-red'])
  })

  it('is valid with the longest username as a seat name, and a seat added later skips no number', () => {
    const longest = 'a'.repeat(32)
    const draft = defaultDraft(DECKS, longest)
    expect(problemCount(problemsOf(draft, DECKS))).toBe(0)
    expect(addSeat(draft, DECKS).seats.map((seat) => seat.name)).toEqual([longest, 'Player 2', 'Player 1'])
  })
})

describe('adding and removing seats', () => {
  it.each(seatCounts)('grows to %i seats, each with its own id and a distinct default name', (count) => {
    const draft = grown(count)
    expect(draft.seats).toHaveLength(count)
    expect(new Set(draft.seats.map((seat) => seat.id)).size).toBe(count)
    expect(new Set(draft.seats.map((seat) => seat.name)).size).toBe(count)
  })

  it('spreads new seats over the decks, using the least used first', () => {
    expect(grown(5).seats.map((seat) => seat.deck)).toEqual([
      'mono-green',
      'mono-red',
      'mono-white',
      'mono-green',
      'mono-red',
    ])
  })

  it(`does not grow past ${MAX_SEATS} seats`, () => {
    const full = grown(MAX_SEATS)
    expect(addSeat(full, DECKS)).toBe(full)
  })

  it(`does not shrink below ${MIN_SEATS} seats`, () => {
    const start = defaultDraft(DECKS)
    const [first] = start.seats
    expect(removeSeat(start, first?.id ?? -1)).toBe(start)
  })

  it('removes the seat asked for and keeps what was typed into the others', () => {
    let draft = grown(3)
    const [a, b, c] = draft.seats
    draft = editSeat(draft, a?.id ?? -1, { name: 'Ana' })
    draft = editSeat(draft, c?.id ?? -1, { name: 'Cy', deck: 'mono-red' })
    draft = removeSeat(draft, b?.id ?? -1)
    expect(draft.seats.map((seat) => [seat.name, seat.deck])).toEqual([
      ['Ana', 'mono-green'],
      ['Cy', 'mono-red'],
    ])
  })

  it('never hands out a name another seat is using, even after a seat in the middle went', () => {
    let draft = grown(3)
    draft = removeSeat(draft, draft.seats[1]?.id ?? -1)
    draft = addSeat(draft, DECKS)
    expect(draft.seats.map((seat) => seat.name)).toEqual(['Player 1', 'Player 3', 'Player 2'])
  })

  it('never reuses the id of a removed seat', () => {
    let draft = grown(3)
    const gone = draft.seats[2]?.id
    draft = removeSeat(draft, gone ?? -1)
    draft = addSeat(draft, DECKS)
    expect(draft.seats.map((seat) => seat.id)).not.toContain(gone)
  })

  it('does not touch the draft it is given', () => {
    const start = defaultDraft(DECKS)
    const before = structuredClone(start)
    addSeat(start, DECKS)
    editSeat(start, 0, { name: 'x' })
    removeSeat(grown(3), 0)
    expect(start).toEqual(before)
  })
})

describe('nameProblem', () => {
  const emoji = '\u{1F600}'
  it.each([
    ['a plain name', 'Ana', null],
    ['a name with spaces inside', 'Ana Maria', null],
    ['a name that is exactly the longest allowed', 'x'.repeat(MAX_NAME_CHARS), null],
    ['a long name padded with spaces that are trimmed away', ` ${'x'.repeat(MAX_NAME_CHARS)} `, null],
    ['characters counted as characters, not UTF-16 units', emoji.repeat(MAX_NAME_CHARS), null],
    ['an accented name', 'Zoë Ångström', null],
  ])('accepts %s', (_what, name, problem) => {
    expect(nameProblem(name)).toBe(problem)
  })

  it.each([
    ['nothing', '', /enter a name/i],
    ['only spaces', '    ', /enter a name/i],
    [
      'a name one character too long',
      'x'.repeat(MAX_NAME_CHARS + 1),
      /at most 32 characters \(this one has 33\)/i,
    ],
    ['too many characters of more than one UTF-16 unit', emoji.repeat(MAX_NAME_CHARS + 1), /has 33/],
    ['a tab', 'a\tb', /control or invisible/i],
    ['a newline', 'a\nb', /control or invisible/i],
    ['the delete character', 'a\u007fb', /control or invisible/i],
    ['a C1 control character', 'a\u0085b', /control or invisible/i],
    ['a zero-width space', 'a​b', /control or invisible/i],
    ['a right-to-left override', 'a‮b', /control or invisible/i],
    ['a bidi isolate', 'a⁦b', /control or invisible/i],
    ['a word joiner', 'a⁠b', /control or invisible/i],
    ['a byte order mark', 'a﻿b', /control or invisible/i],
  ])('refuses %s', (_what, name, problem) => {
    expect(nameProblem(name)).toMatch(problem)
  })
})

describe('problemsOf', () => {
  it('marks only the seats that are wrong, by id', () => {
    let draft = chooseLinks(grown(3), true)
    const [a, b, c] = draft.seats
    draft = editSeat(draft, a?.id ?? -1, { name: '' })
    draft = editSeat(draft, c?.id ?? -1, { deck: 'gone' })
    const problems = problemsOf(draft, DECKS)
    expect(problems.get(a?.id ?? -1)).toEqual({ name: expect.stringMatching(/enter a name/i), deck: null })
    expect(problems.get(b?.id ?? -1)).toEqual({ name: null, deck: null })
    expect(problems.get(c?.id ?? -1)).toEqual({ name: null, deck: 'Choose a deck.' })
    expect(problemCount(problems)).toBe(2)
  })

  it('counts a seat once even when both fields are wrong', () => {
    const draft = editSeat(defaultDraft(DECKS), 0, { name: '', deck: '' })
    expect(problemCount(problemsOf(draft, DECKS))).toBe(1)
  })
})

describe('resizeSeats', () => {
  it.each(seatCounts)('makes exactly %i seats, from either side', (count) => {
    expect(resizeSeats(defaultDraft(DECKS), DECKS, count).seats).toHaveLength(count)
    expect(resizeSeats(grown(MAX_SEATS), DECKS, count).seats).toHaveLength(count)
  })

  it.each([-3, 0, 1, 99, 2.9, Number.NaN])('keeps %s between the fewest and the most seats', (count) => {
    const seats = resizeSeats(defaultDraft(DECKS), DECKS, count).seats.length
    expect(seats).toBeGreaterThanOrEqual(MIN_SEATS)
    expect(seats).toBeLessThanOrEqual(MAX_SEATS)
  })

  it('keeps what was typed in the seats that stay, and drops the last ones', () => {
    let draft = grown(4)
    draft = editSeat(draft, draft.seats[0]?.id ?? -1, { name: 'Ana', deck: 'mono-white' })
    draft = resizeSeats(draft, DECKS, 2)
    expect(draft.seats.map((seat) => [seat.name, seat.deck])).toEqual([
      ['Ana', 'mono-white'],
      ['Player 2', 'mono-red'],
    ])
    expect(resizeSeats(draft, DECKS, 3).seats.map((seat) => seat.name)).toEqual([
      'Ana',
      'Player 2',
      'Player 1',
    ])
  })

  it('returns the same draft when the count is already right', () => {
    const draft = defaultDraft(DECKS)
    expect(resizeSeats(draft, DECKS, 2)).toBe(draft)
  })
})

describe('choosing', () => {
  it('changes only the visibility, and only the link switch', () => {
    const start = defaultDraft(DECKS)
    expect(chooseVisibility(start, 'invite')).toEqual({ ...start, visibility: 'invite' })
    expect(chooseLinks(start, true)).toEqual({ ...start, links: true })
    expect(start.visibility).toBe('public')
    expect(start.links).toBe(false)
  })
})

describe('toRequest', () => {
  it.each(seatCounts)(
    'builds the exact body for %i seats, only the creator’s held and only the creator’s with a deck',
    (count) => {
      const draft = grown(count)
      expect(toRequest(draft)).toEqual({
        format: 'normal',
        seats: draft.seats.map((seat, index) =>
          index === 0 ? { name: seat.name, deck: seat.deck } : { name: `Player ${index + 1}` },
        ),
        visibility: 'public',
        held: 1,
      })
      expect(Object.keys(toRequest(draft)).sort()).toEqual(['format', 'held', 'seats', 'visibility'])
      const [creator, ...open] = toRequest(draft).seats
      expect(Object.keys(creator ?? {}).sort()).toEqual(['deck', 'name'])
      // Open seats name no deck at all, so the server gives them none until their player chooses.
      for (const seat of open) expect(Object.keys(seat)).toEqual(['name'])
    },
  )

  it('sends the visibility that was chosen', () => {
    expect(toRequest(chooseVisibility(defaultDraft(DECKS), 'invite')).visibility).toBe('invite')
  })

  it('sends the open seats as Player N whatever their fields hold, since the server never reads them', () => {
    let draft = grown(3)
    draft = editSeat(draft, draft.seats[1]?.id ?? -1, { name: '' })
    expect(toRequest(draft).seats.map((seat) => seat.name)).toEqual(['Player 1', 'Player 2', 'Player 3'])
  })

  it.each(seatCounts)(
    'builds the body of a link for every seat for %i seats: all held, invite only',
    (count) => {
      const draft = chooseLinks(chooseVisibility(grown(count), 'public'), true)
      expect(toRequest(draft)).toEqual({
        format: 'normal',
        seats: draft.seats.map((seat) => ({ name: seat.name, deck: seat.deck })),
        visibility: 'invite',
      })
      expect('held' in toRequest(draft)).toBe(false)
    },
  )

  it('trims names, as the server does', () => {
    const draft = editSeat(defaultDraft(DECKS), 0, { name: '  Ana \t' })
    expect(toRequest(draft).seats[0]).toEqual({ name: 'Ana', deck: 'mono-green' })
  })

  it('keeps seat order', () => {
    let draft = chooseLinks(grown(3), true)
    draft = editSeat(draft, draft.seats[0]?.id ?? -1, { name: 'first' })
    draft = editSeat(draft, draft.seats[2]?.id ?? -1, { name: 'third' })
    expect(toRequest(draft).seats.map((seat) => seat.name)).toEqual(['first', 'Player 2', 'third'])
  })
})
