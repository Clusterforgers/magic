// Goal: the create form fills the seats after the creator's with bots: the count runs from none to every other seat and follows the seat count, a Normal bot plays the deck the form gave its seat and a Commander bot a playable precon (Verified first), bot seats are held and never handed out as links, a table without a usable precon asks to go without bots, and a link per seat has none.
import { screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { preconItem } from '@/features/precons/fakePrecons'
import { loadInvites } from '@/lib/invites'
import { DECKS } from '@/test-utils/decks'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { openCreateForm, typeSeats } from '@/test-utils/createForm'
import { MADE_GAME } from '@/test-utils/fakeServer'
import { botsOf, chooseBots, chooseLinks, defaultDraft, pickBotPrecons, resizeSeats, toRequest } from './form'

const bots = () => screen.getByLabelText('Bots')
const create = () => screen.getByRole('button', { name: 'Create game' })

describe('the bots of the form', () => {
  it.each([
    [2, 1, 1],
    [4, 3, 3],
    [4, 9, 3],
    [4, 0, 0],
  ])('at %i seats, %i asked for are %i', (seats, asked, made) => {
    const draft = chooseBots(resizeSeats(defaultDraft(DECKS, undefined, 'normal'), DECKS, seats), asked)
    expect(botsOf(draft)).toBe(made)
    const request = toRequest(draft)
    expect(request.held).toBe(1 + made)
    expect(request.seats.filter((seat) => seat.bot === true)).toHaveLength(made)
    request.seats.slice(1, 1 + made).forEach((seat, at) => {
      expect(seat).toEqual({ name: 'Bot', deck: draft.seats[at + 1]?.deck, bot: true })
    })
    expect(request.seats.slice(1 + made).every((seat) => seat.bot === undefined && seat.deck === undefined)).toBe(true)
  })

  it('has none with a link per seat', () => {
    const draft = chooseLinks(chooseBots(defaultDraft(DECKS, undefined, 'normal'), 1), true)
    expect(botsOf(draft)).toBe(0)
    expect(toRequest(draft).seats.some((seat) => seat.bot === true)).toBe(false)
  })

  it('gives Commander bots the decks it is handed', () => {
    const draft = chooseBots(defaultDraft(DECKS, undefined, 'commander'), 2)
    const request = toRequest(draft, ['precon:a', 'precon:b'])
    expect(request.seats.map((seat) => seat.deck)).toEqual([undefined, 'precon:a', 'precon:b', undefined])
  })
})

describe('pickBotPrecons', () => {
  const verified = preconItem({ code: 'v' })
  const read = preconItem({ code: 'r', state: 'read', verified: false })
  const blocked = preconItem({ code: 'x', playable: false })
  const freeform = preconItem({ code: 'f', format: 'freeform' })

  it.each([
    [[verified, read], 1, ['precon:v']],
    [[read, verified], 2, ['precon:v', 'precon:r']],
    [[verified], 3, ['precon:v', 'precon:v', 'precon:v']],
    [[blocked, freeform, read], 1, ['precon:r']],
    [[blocked, freeform], 2, []],
    [[], 1, []],
  ])('from %j picks %i as %j', (list, count, picked) => {
    expect(pickBotPrecons(list, count, () => 0.5)).toEqual(picked)
  })
})

describe('the Bots field', () => {
  it('runs from none to every other seat and follows the seat count', async () => {
    const { user } = await openCreateForm({}, 'normal')
    expect(bots()).toHaveValue('0')
    expect(within(bots()).getAllByRole('option').map((option) => option.textContent)).toEqual([
      'No bots',
      '1 — every other seat',
    ])
    await typeSeats(user, 4)
    await user.selectOptions(bots(), '3')
    await typeSeats(user, 3)
    expect(bots()).toHaveValue('2')
  })

  it('makes a Normal game whose bot seats are held and not handed out', async () => {
    const { user, server } = await openCreateForm({}, 'normal')
    await typeSeats(user, 3)
    await user.selectOptions(bots(), '1')
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    const body = server.creates[0] as { held: number; seats: { bot?: boolean; deck?: string }[] }
    expect(body.held).toBe(2)
    expect(body.seats[1]).toMatchObject({ bot: true, deck: DECKS[1]?.id })
    expect(body.seats[2]?.bot).toBeUndefined()
    await waitFor(() => expect(loadInvites(MADE_GAME)).not.toBeNull())
    expect(loadInvites(MADE_GAME)?.seats.map((seat) => seat.seat)).not.toContain(1)
  })

  it('gives Commander bots a precon each', async () => {
    const precons = [preconItem({ code: 'one' })]
    const { user, server } = await openCreateForm({ precons: () => jsonResponse(200, { precons }) }, 'default')
    await user.selectOptions(bots(), '3')
    await user.click(create())
    await waitFor(() => expect(server.creates).toHaveLength(1))
    const body = server.creates[0] as { held: number; seats: { bot?: boolean; deck?: string }[] }
    expect(body.held).toBe(4)
    expect(body.seats.slice(1).map((seat) => seat.deck)).toEqual(['precon:one', 'precon:one', 'precon:one'])
  })

  it('asks to go without bots when no precon can be played', async () => {
    const { user, server } = await openCreateForm({}, 'default')
    await user.selectOptions(bots(), '1')
    await user.click(create())
    expect(await screen.findByText(/No Commander precon is available for the bots/)).toBeInTheDocument()
    expect(server.creates).toHaveLength(0)
  })
})
