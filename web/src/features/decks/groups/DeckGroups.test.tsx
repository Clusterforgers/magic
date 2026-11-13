// Goal: the decks page shows the account's decks in collapsible groups in the owner's order with Ungrouped last, keeps the collapsed state on the server, offers the Fun / Hardcore / Dungeon presets to an account without groups, creates, renames, reorders and deletes groups, moves a deck with its Group menu or by dropping it on a group, and falls back to one list when the groups cannot be had.
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import type { DeckGroupView } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { DecksPage } from '../DecksPage'
import { deckItem, startDecks } from '../fakeDecks'
import { DECK_DRAG_TYPE } from './drag'
import { groupView, startGroups } from './fakeGroups'
import { groupOf, moved, sections } from './grouping'

const DECKS = [
  deckItem({ id: 'd1', name: 'Atraxa' }),
  deckItem({ id: 'd2', name: 'Krenko' }),
  deckItem({ id: 'd3', name: 'Muldrotha' }),
]

async function renderPage(groups: DeckGroupView[] = [], failing?: 'network') {
  const server = startDecks({ decks: DECKS })
  const fake = startGroups(groups, failing)
  const accounts = startAccounts({ users: { Ada_L: 'correct horse battery' }, signedIn: 'Ada_L' })
  const store = createAuthStore({ api: accounts.api })
  await renderInRouter(
    <AuthProvider store={store}>
      <DecksPage api={server.api} clock={() => Date.parse('2026-10-18T12:00:00Z')} groupsApi={fake.api} />
    </AuthProvider>,
  )
  await screen.findByRole('heading', { level: 1, name: 'Decks' })
  return { fake, user: userEvent.setup() }
}

const heading = (name: RegExp) => screen.getByRole('button', { name })
const decksIn = (group: string): string[] =>
  within(screen.getByRole('list', { name: `Decks in ${group}` }))
    .getAllByRole('listitem')
    .filter((item) => item.dataset.deck !== undefined)
    .map((item) => item.dataset.deck ?? '')

describe('grouping', () => {
  const items = [{ id: 'a' }, { id: 'b' }, { id: 'c' }]
  it.each([
    [[], false, [['ungrouped', ['a', 'b', 'c']]]],
    [
      [groupView('g', 'G', ['c', 'a'])],
      false,
      [
        ['g', ['a', 'c']],
        ['ungrouped', ['b']],
      ],
    ],
    [[groupView('g', 'G', ['a', 'b', 'c'])], false, [['g', ['a', 'b', 'c']]]],
    [
      [groupView('g', 'G', ['a', 'b', 'c'])],
      true,
      [
        ['g', ['a', 'b', 'c']],
        ['ungrouped', []],
      ],
    ],
    [
      [groupView('e', 'E'), groupView('g', 'G', ['gone', 'b'])],
      false,
      [
        ['g', ['b']],
        ['ungrouped', ['a', 'c']],
      ],
    ],
  ] as const)('sections of %j (keep empty: %s)', (groups, keep, expected) => {
    const got = sections(items, groups, keep).map((s) => [s.key, s.items.map((i) => i.id)])
    expect(got).toEqual(expected)
  })

  it('finds a deck’s group and moves a group inside the list', () => {
    const groups = [groupView('x', 'X'), groupView('y', 'Y', ['d']), groupView('z', 'Z')]
    expect(groupOf(groups, 'd')).toBe('y')
    expect(groupOf(groups, 'e')).toBeNull()
    expect(moved(groups, 'x', 1).map((g) => g.id)).toEqual(['y', 'x', 'z'])
    expect(moved(groups, 'x', -1).map((g) => g.id)).toEqual(['x', 'y', 'z'])
    expect(moved(groups, 'z', 5).map((g) => g.id)).toEqual(['x', 'y', 'z'])
  })
})

describe('the decks page in groups', () => {
  it('lists the groups in order, Ungrouped last, and folds a collapsed one', async () => {
    await renderPage([groupView('g1', 'Fun', ['d2']), groupView('g2', 'Hardcore', ['d3'], true)])
    expect(await screen.findByRole('list', { name: 'Decks in Fun' })).toBeInTheDocument()
    const headings = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent)
    expect(headings).toEqual(['Fun1 deck', 'Hardcore1 deck', 'Ungrouped1 deck'])
    expect(decksIn('Fun')).toEqual(['d2'])
    expect(decksIn('Ungrouped')).toEqual(['d1'])
    expect(heading(/^Hardcore/)).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('list', { name: 'Decks in Hardcore' })).toBeNull()
  })

  it('keeps a collapse on the server and opens the group again', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun', ['d2'])])
    await user.click(await screen.findByRole('button', { name: /^Fun/ }))
    expect(heading(/^Fun/)).toHaveAttribute('aria-expanded', 'false')
    expect(fake.calls).toContain('change g1 {"collapsed":true}')
    await user.click(heading(/^Fun/))
    expect(fake.calls).toContain('change g1 {"collapsed":false}')
    expect(decksIn('Fun')).toEqual(['d2'])
  })

  it('offers the presets to an account without groups and adds them in order', async () => {
    const { fake, user } = await renderPage()
    const add = await screen.findByRole('button', { name: 'Add Fun, Hardcore and Dungeon' })
    expect(screen.getByRole('list', { name: 'Your decks' })).toBeInTheDocument()
    await user.click(add)
    await screen.findByRole('list', { name: 'Decks in Ungrouped' })
    await waitFor(() => expect(fake.groups.map((g) => g.name)).toEqual(['Fun', 'Hardcore', 'Dungeon']))
    expect(screen.queryByRole('button', { name: /^Add Fun/ })).toBeNull()
  })

  it('creates and renames a group with the name dialog, refusing an empty name', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun')])
    await user.click(await screen.findByRole('button', { name: 'New group' }))
    await user.click(screen.getByRole('button', { name: 'Create group' }))
    expect(screen.getByText('Give the group a name.')).toBeInTheDocument()
    await user.type(screen.getByLabelText('Group name'), 'Dungeon')
    await user.click(screen.getByRole('button', { name: 'Create group' }))
    expect(await screen.findByRole('button', { name: /^Dungeon/ })).toBeInTheDocument()
    expect(fake.calls).toContain('create Dungeon')

    await user.click(screen.getByRole('button', { name: 'Actions for the group Fun' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Rename' }))
    const field = await screen.findByLabelText('Group name')
    await user.clear(field)
    await user.type(field, 'Casual')
    await user.click(screen.getByRole('button', { name: 'Rename' }))
    expect(await screen.findByRole('button', { name: /^Casual/ })).toBeInTheDocument()
    expect(fake.calls).toContain('change g1 {"name":"Casual"}')
  })

  it('moves a group down and deletes one, its decks going to Ungrouped', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun', ['d1']), groupView('g2', 'Hardcore')])
    await user.click(await screen.findByRole('button', { name: 'Actions for the group Fun' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Move down' }))
    await waitFor(() => expect(fake.calls).toContain('reorder g2,g1'))
    expect(screen.getAllByRole('heading', { level: 2 })[0]).toHaveTextContent('Hardcore')

    await user.click(screen.getByRole('button', { name: 'Actions for the group Fun' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Delete group' }))
    await waitFor(() => expect(screen.queryByRole('button', { name: /^Fun/ })).toBeNull())
    expect(decksIn('Ungrouped')).toEqual(['d1', 'd2', 'd3'])
    expect(fake.calls).toContain('remove g1')
  })

  it('moves a deck with its Group menu and by dropping it on a group', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun'), groupView('g2', 'Hardcore')])
    await screen.findByRole('button', { name: /^Fun/ })
    await user.click(screen.getByRole('button', { name: 'Move Krenko to a group' }))
    await user.click(await screen.findByRole('menuitemradio', { name: 'Hardcore' }))
    await waitFor(() => expect(decksIn('Hardcore')).toEqual(['d2']))
    expect(fake.calls).toContain('file d2 g2')

    const data = new Map<string, string>([[DECK_DRAG_TYPE, 'd3']])
    const dataTransfer = {
      types: [DECK_DRAG_TYPE],
      getData: (type: string) => data.get(type) ?? '',
      setData: () => undefined,
      dropEffect: 'none',
    }
    const fun = screen.getByRole('button', { name: /^Fun/ }).closest('section')
    if (fun === null) throw new Error('no section')
    act(() => {
      fireEvent.dragOver(fun, { dataTransfer })
      fireEvent.drop(fun, { dataTransfer })
    })
    await waitFor(() => expect(decksIn('Fun')).toEqual(['d3']))
    expect(fake.calls).toContain('file d3 g1')
    expect(screen.getByRole('status')).toHaveTextContent('Moved “Muldrotha” to Fun.')
  })

  it('shows one list without group controls when the groups cannot be had', async () => {
    await renderPage([groupView('g1', 'Fun')], 'network')
    expect(await screen.findByRole('list', { name: 'Your decks' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'New group' })).toBeNull()
    expect(screen.queryByRole('button', { name: /Move .* to a group/ })).toBeNull()
  })
})
