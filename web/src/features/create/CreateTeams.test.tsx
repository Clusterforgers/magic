// Goal: the game mode of the create form fits the seats to the mode (Two-Headed Giant takes 4, Teams an even count from 4, Free-for-all keeps them), refuses a count the mode cannot play, and sends the engine's default teams in the house rules, in both formats and with or without a link per seat.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { DECKS } from '@/test-utils/decks'
import { chooseSeatLinks, openCreateForm, typeSeats } from '@/test-utils/createForm'
import type { GameMode } from '@/lib/teams'
import { chooseFormat, chooseMode, defaultDraft, modeProblem, resizeSeats, toRequest } from './form'

describe('the mode in the form model', () => {
  it.each<[GameMode, number, number]>([
    ['two_headed_giant', 2, 4],
    ['two_headed_giant', 6, 4],
    ['teams', 2, 4],
    ['teams', 5, 6],
    ['teams', 6, 6],
    ['free_for_all', 5, 5],
  ])('%s from %i seats has %i', (mode, from, to) => {
    const draft = chooseMode(resizeSeats(defaultDraft(DECKS), DECKS, from), DECKS, mode)
    expect(draft.seats).toHaveLength(to)
    expect(modeProblem(draft)).toBeNull()
  })

  it.each<[GameMode, number, string | null]>([
    ['two_headed_giant', 5, 'Two-Headed Giant needs exactly 4 seats.'],
    ['teams', 5, 'Teams needs an even number of seats, 4 or more.'],
    ['teams', 2, 'Teams needs an even number of seats, 4 or more.'],
    ['teams', 8, null],
    ['free_for_all', 3, null],
  ])('%s with %i seats is refused with %s', (mode, count, problem) => {
    const draft = resizeSeats(chooseMode(defaultDraft(DECKS), DECKS, mode), DECKS, count)
    expect(modeProblem(draft)).toBe(problem)
  })

  it.each<[GameMode, number, number[] | undefined]>([
    ['two_headed_giant', 4, [0, 0, 1, 1]],
    ['teams', 4, [0, 1, 0, 1]],
    ['teams', 6, [0, 1, 0, 1, 0, 1]],
    ['free_for_all', 4, undefined],
  ])('%s with %i seats sends the teams %j', (mode, count, seats) => {
    const draft = resizeSeats(chooseMode(defaultDraft(DECKS), DECKS, mode), DECKS, count)
    const request = toRequest(draft)
    if (seats === undefined) expect(request).not.toHaveProperty('house_rules')
    else expect(request.house_rules?.teams).toEqual({ mode, seats })
  })

  it('keeps the seats of a team mode when the format changes', () => {
    const giant = chooseMode(defaultDraft(DECKS), DECKS, 'two_headed_giant')
    expect(chooseFormat(giant, DECKS, 'normal').seats).toHaveLength(4)
  })
})

describe('the mode on the page', () => {
  it('starts on Free-for-all and sends no teams', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    expect(screen.getByRole('radio', { name: /^Free-for-all/ })).toBeChecked()
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(server.creates[0]).not.toHaveProperty('house_rules')
  })

  it('makes Two-Headed Giant four seats that cannot change, and sends its teams with the typed rules', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    await user.click(screen.getByRole('radio', { name: /^Two-Headed Giant/ }))
    const seats = screen.getByLabelText('Seats')
    expect(seats).toHaveValue(4)
    expect(seats).toHaveAttribute('readonly')
    expect(screen.getByText('Two-Headed Giant is played by 4 players.')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(server.creates[0]).toMatchObject({
      house_rules: { teams: { mode: 'two_headed_giant', seats: [0, 0, 1, 1] } },
    })
  })

  it('refuses an odd count of Teams until the seats fit, then sends alternating teams', async () => {
    const { server, user } = await openCreateForm({}, 'default')
    await user.click(screen.getByRole('radio', { name: /^Teams/ }))
    await typeSeats(user, 5)
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(server.creates).toHaveLength(0)
    expect(screen.getByText('Teams needs an even number of seats, 4 or more.')).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /^Teams/ })).toHaveFocus()
    await typeSeats(user, 6)
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(server.creates[0]).toMatchObject({
      house_rules: { teams: { mode: 'teams', seats: [0, 1, 0, 1, 0, 1] } },
    })
  })

  it('refuses a seat removed from Two-Headed Giant when every seat has its own link', async () => {
    const { server, user } = await openCreateForm()
    await chooseSeatLinks(user)
    await user.click(screen.getByRole('radio', { name: /^Two-Headed Giant/ }))
    await user.click(screen.getAllByRole('button', { name: /^Remove/ })[0] as HTMLElement)
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    expect(server.creates).toHaveLength(0)
    expect(screen.getByText('Two-Headed Giant needs exactly 4 seats.')).toBeInTheDocument()
  })
})
