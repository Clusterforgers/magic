// Goal: a run in the list shows its party and how far down the ladder it got, one skull per boss with the beaten ones marked; runs under way lead the list, finished runs (victory, defeat) sit in a collapsed "Past runs" section, and abandoned runs are not shown.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DelveSummary } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { fakeDelves, delveView } from './fakeDelves'
import { DelveList, DelveRow, splitRuns } from './DelvesPage'

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
