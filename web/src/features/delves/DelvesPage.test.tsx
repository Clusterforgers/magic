// Goal: a run in the list shows its party and how far down the ladder it got, one skull per boss with the beaten ones marked; runs under way lead the list, finished runs (victory, defeat) sit in a collapsed "Past runs" section, abandoned runs are not shown, and the new-run form offers the three Heats as worded tiers and the timers with Relaxed as its default and sends the ones that are chosen.
import { screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import type { DelveSummary } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { fakeDelves, delveView } from './fakeDelves'
import { DelveList, DelveRow, NewDelveForm, splitRuns } from './DelvesPage'

const summary = (id: string, status: string): DelveSummary => ({
  id,
  code: id,
  theme: `Run ${id}`,
  status,
  heroes: ['Ana'],
  cleared: 1,
  bosses: 5,
})

it('shows the bosses beaten as skulls', async () => {
  await renderInRouter(
    <ul>
      <DelveRow delve={{ ...summary('d1', 'map'), heroes: ['Ana', 'Bo'], cleared: 2 }} />
    </ul>,
  )
  const track = screen.getByRole('img', { name: '2 of 5 bosses beaten' })
  const skulls = [...track.querySelectorAll('[data-beaten]')].map((s) => s.getAttribute('data-beaten'))
  expect(skulls).toEqual(['true', 'true', 'false', 'false', 'false'])
  expect(screen.getByText('Ana, Bo')).toBeInTheDocument()
  expect(screen.getByText('Choosing a path')).toBeInTheDocument()
})

describe('the list of runs', () => {
  const runs = [
    summary('a', 'fight'),
    summary('b', 'victory'),
    summary('c', 'abandoned'),
    summary('d', 'lobby'),
    summary('e', 'defeat'),
  ]

  it.each([
    [['fight', 'lobby', 'map', 'room'], 'active'],
    [['victory', 'defeat'], 'past'],
    [['abandoned'], 'hidden'],
  ] as const)('%j runs are %s', (statuses, where) => {
    const { active, past } = splitRuns(statuses.map((status, at) => summary(String(at), status)))
    expect(active).toHaveLength(where === 'active' ? statuses.length : 0)
    expect(past).toHaveLength(where === 'past' ? statuses.length : 0)
  })

  it('draws runs under way first and the finished ones in a collapsed Past runs', async () => {
    const client = { ...fakeDelves(delveView()), list: async () => ({ delves: runs }) }
    await renderInRouter(<DelveList client={client} />)
    const active = within(await screen.findByRole('list', { name: 'Your delves' }))
    expect(active.getByText('Run a')).toBeInTheDocument()
    expect(active.getByText('Run d')).toBeInTheDocument()
    expect(active.queryByText('Run b')).toBeNull()
    expect(screen.queryByText('Run c')).toBeNull()
    const past = document.querySelector('details[data-slot="past-runs"]')
    expect(past?.hasAttribute('open')).toBe(false)
    expect(within(past as HTMLElement).getByText('Past runs (2)')).toBeInTheDocument()
    expect(within(past as HTMLElement).getByText('Run b')).toBeInTheDocument()
  })
})

describe('the new run form', () => {
  const open = async () => {
    const bodies: unknown[] = []
    const client = {
      ...fakeDelves(delveView()),
      create: async (body: unknown) => {
        bodies.push(body)
        return delveView()
      },
    }
    await renderInRouter(<NewDelveForm client={client as never} />)
    return { bodies, user: userEvent.setup() }
  }

  it('offers the timers with Relaxed chosen and says what each one means', async () => {
    await open()
    const timers = screen.getByLabelText('Timers')
    expect(timers).toHaveValue('relaxed')
    const words = within(timers)
      .getAllByRole('option')
      .map((option) => [option.getAttribute('value'), option.textContent])
    expect(words).toEqual([
      ['relaxed', 'Relaxed: 30 seconds to respond, 3 minutes to think'],
      ['quick', 'Quick: 10 seconds to respond, 1 minute to think'],
      ['off', 'Off: no time limit in any fight'],
    ])
  })

  it('offers the three Heats as tiers, Casual chosen, with what the chosen one asks in plain words', async () => {
    const { bodies, user } = await open()
    const heat = screen.getByLabelText('Heat')
    expect(heat).toHaveValue('0')
    expect(within(heat).getAllByRole('option').map((option) => option.textContent)).toEqual([
      'Heat 0 — Casual',
      'Heat 1 — Seasoned',
      'Heat 2 — Heroic',
    ])
    expect(heat).toHaveAccessibleDescription(/^Heat 0 — Casual: any deck can win with some luck\./)
    await user.selectOptions(heat, '2')
    expect(heat).toHaveAccessibleDescription(
      /^Heat 2 — Heroic: needs a team of decks that work together and suit the format\./,
    )
    await user.click(screen.getByRole('button', { name: 'Create delve' }))
    expect(bodies).toEqual([{ heat: 2, timers: 'relaxed' }])
  })

  it.each([
    ['the default', undefined, 'relaxed'],
    ['quick', 'quick', 'quick'],
    ['off', 'off', 'off'],
  ])('sends the timers it was left with or given: %s', async (_name, choice, sent) => {
    const { bodies, user } = await open()
    if (choice !== undefined) await user.selectOptions(screen.getByLabelText('Timers'), choice)
    await user.click(screen.getByRole('button', { name: 'Create delve' }))
    expect(bodies).toEqual([{ heat: 0, timers: sent }])
  })
})
