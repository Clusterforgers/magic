// Goal: after a hero answers, the run page says what happened: each hero's choice with its gains and costs and how a gamble fell, the viewer first as You; it shows nothing until someone answered, and stays through the next fight and the vote but not in the lobby or at the end.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderInRouter } from '@/test-utils/router'
import type { Resolved, Run } from '@/protocol/generated'
import { DelvePage } from './DelvePage'
import { delveView, fakeDelves, hero } from './fakeDelves'
import { pickerDecks } from './fakePickerDecks'
import { ResultPanel } from './ResultPanel'

const heroes = [
  hero({ name: 'Ana' }),
  hero({ name: 'Bo', user: 'u1' }),
  hero({ name: 'Cy', user: null, bot: 'Normal' }),
]
const results: Resolved[] = [
  { hero: 1, what: 'chose “Push past”', outcomes: [{ Life: -8 }, { Gold: 40 }], rolls: [] },
  { hero: 0, what: 'chose “Reach”', outcomes: ['Relic'], rolls: [{ odds: 50, won: true }] },
  { hero: 2, what: 'chose “Turn back”', outcomes: [], rolls: [] },
]

describe('the result panel', () => {
  const view = delveView({}, { status: 'Room', heroes, results })

  it('lists each hero’s answer with the viewer first', () => {
    render(<ResultPanel view={view} />)
    const region = screen.getByRole('status', { name: 'What happened' })
    const rows = within(region).getAllByRole('listitem')
    expect(rows.map((row) => row.getAttribute('aria-label'))).toEqual([
      'You chose “Reach”: Won the 50% gamble, a relic.',
      'Bo chose “Push past”: −8 life, +40 gold.',
      'Cy chose “Turn back”.',
    ])
  })

  it('shows the chips of what was gained and paid, and how a gamble fell', () => {
    render(<ResultPanel view={view} />)
    const region = screen.getByRole('status')
    expect(within(region).getByText('Won the 50% gamble')).toBeInTheDocument()
    expect(within(region).getByText('No effect')).toBeInTheDocument()
    expect(within(region).getByText('−8 life').closest('[data-chip]')).toHaveAttribute('data-tone', 'cost')
    expect(within(region).getByText('+40 gold').closest('[data-chip]')).toHaveAttribute('data-tone', 'gain')
    expect(within(region).getByText('a relic').closest('[data-chip]')).toHaveAttribute('data-chip', 'relic')
  })

  it('shows nothing before anyone answered', () => {
    render(<ResultPanel view={delveView({}, { status: 'Room', heroes, results: [] })} />)
    expect(screen.queryByRole('status')).toBeNull()
  })
})

describe('the run page', () => {
  const show = async (status: Run['status']) => {
    await renderInRouter(
      <DelvePage
        id="d1"
        client={fakeDelves(delveView({}, { status, at: 0, cleared: [0], heroes, results }))}
        loadDecks={pickerDecks}
        find={async () => null}
      />,
    )
  }

  it.each([['Room'], ['Map'], [{ Fight: { game: 'g1' } }]] as const)(
    'shows what happened in the camp at %j',
    async (status) => {
      await show(status)
      expect(await screen.findByRole('status', { name: 'What happened' })).toBeInTheDocument()
    },
  )

  it.each([['Lobby'], ['Victory'], ['Defeat']] as const)('leaves the panel out at %s', async (status) => {
    await show(status)
    await screen.findByRole('heading', { name: 'Chronicle' })
    expect(screen.queryByRole('status', { name: 'What happened' })).toBeNull()
  })
})
