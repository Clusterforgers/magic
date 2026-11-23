// Goal: a descent into another theme shows "The party descends into …" on the new floor until its first room is cleared, never on a floor of the same theme or once the run is over, and the run page and its ladder take the floor's theme name and look.
import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { DelveView, Run, Status } from '@/protocol/generated'
import { renderInRouter } from '@/test-utils/router'
import { DelvePage } from './DelvePage'
import { DescentNotice } from './DescentNotice'
import { delveView, fakeDelves } from './fakeDelves'
import { pickerDecks } from './fakePickerDecks'
import { descentNotice, floorTheme } from './model'

const SPIRE = 'The Drowned Spire'

function descended(over: Partial<Run> = {}): DelveView {
  return delveView(
    { theme: SPIRE },
    {
      status: { Fight: { game: null } },
      at: 0,
      cleared: [],
      floor: 2,
      floors_cleared: 1,
      floor_theme: 'spire',
      theme_from: 'warrens',
      ...over,
    },
  )
}

describe('the descent notice', () => {
  it('names the new theme on the new floor', () => {
    render(<DescentNotice view={descended()} />)
    expect(screen.getByRole('status')).toHaveTextContent('The party descends into the Drowned Spire')
  })

  it.each<[string, Partial<Run>]>([
    ['a floor of the same theme', { theme_from: undefined }],
    ['a cleared first room', { cleared: [0] }],
    ['a won run', { status: 'Victory' as Status }],
    ['a lost run', { status: 'Defeat' as Status }],
    ['the first floor', { floor: 1, floor_theme: undefined, theme_from: undefined }],
  ])('stays away on %s', (_, over) => {
    const view = descended(over)
    expect(descentNotice(view)).toBeNull()
    const { container } = render(<DescentNotice view={view} />)
    expect(container).toBeEmptyDOMElement()
  })

  it('reads the floor theme, else the run theme', () => {
    expect(floorTheme(descended().run)).toBe('spire')
    expect(floorTheme(descended({ floor_theme: undefined }).run)).toBe('warrens')
  })
})

describe('the run page on a new theme', () => {
  it('shows the notice, the theme name on the ladder and the theme look', async () => {
    const view = descended({ status: 'Map', at: 0, cleared: [] })
    await renderInRouter(
      <DelvePage id="d1" client={fakeDelves(view)} loadDecks={pickerDecks} find={async () => null} />,
    )
    expect(await screen.findByText('The party descends into the Drowned Spire')).toBeInTheDocument()
    expect(within(screen.getByRole('region', { name: 'The descent' })).getByText(SPIRE)).toBeInTheDocument()
    expect(document.querySelector('[data-delve-theme]')).toHaveAttribute('data-delve-theme', 'spire')
  })
})
