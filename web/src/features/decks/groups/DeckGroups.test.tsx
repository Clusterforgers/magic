// Goal: the decks page shows the account's decks in collapsible groups in the owner's order with Ungrouped last, a deck under every group it sits in, keeps the collapsed state on the server, offers the Fun / Hardcore / Dungeon presets to an account without groups, creates, renames, reorders and deletes groups, checks a deck into and out of groups with its Groups menu, makes a new group from that menu, moves, adds or removes a deck by dragging its row, and falls back to one list when the groups cannot be had.
import { act, createEvent, fireEvent, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { AuthProvider } from '@/features/auth/AuthProvider'
import { createAuthStore } from '@/features/auth/store'
import type { DeckGroupView } from '@/protocol/generated'
import { startAccounts } from '@/test-utils/accountsServer'
import { renderInRouter } from '@/test-utils/router'
import { DecksPage } from '../DecksPage'
import { deckItem, startDecks } from '../fakeDecks'
import { DECK_DRAG_FROM_TYPE, DECK_DRAG_TYPE } from './drag'
import { groupView, startGroups } from './fakeGroups'
import { added, groupOf, groupsOf, membership, moved, removed, sections } from './grouping'

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
    // A deck in two groups is in both sections and not in Ungrouped.
    [
      [groupView('g', 'G', ['a', 'b']), groupView('h', 'H', ['b', 'c'])],
      false,
      [
        ['g', ['a', 'b']],
        ['h', ['b', 'c']],
      ],
    ],
    [
      [groupView('g', 'G', ['a']), groupView('h', 'H', ['a'])],
      false,
      [
        ['g', ['a']],
        ['h', ['a']],
        ['ungrouped', ['b', 'c']],
      ],
    ],
  ] as const)('sections of %j (keep empty: %s)', (groups, keep, expected) => {
    const got = sections(items, groups, keep).map((s) => [s.key, s.items.map((i) => i.id)])
    expect(got).toEqual(expected)
  })

  it('adds a deck to a group and removes it from one without touching its others', () => {
    const groups = [groupView('x', 'X', ['d']), groupView('y', 'Y'), groupView('z', 'Z', ['e'])]
    const both = added(groups, 'd', 'y')
    expect(groupsOf(both, 'd')).toEqual(['x', 'y'])
    expect(added(both, 'd', 'y')).toEqual(both)
    expect(groupsOf(removed(both, 'd', 'x'), 'd')).toEqual(['y'])
    expect(removed(both, 'd', 'z')).toEqual(both)
    expect(groupsOf(removed(removed(both, 'd', 'x'), 'd', 'y'), 'd')).toEqual([])
    expect(membership(both).get('d')).toEqual(['x', 'y'])
    expect(membership(both).has('f')).toBe(false)
  })

  it('finds a deck’s groups and moves a group inside the list', () => {
    const groups = [groupView('x', 'X'), groupView('y', 'Y', ['d']), groupView('z', 'Z', ['d'])]
    expect(groupOf(groups, 'd')).toBe('y')
    expect(groupsOf(groups, 'd')).toEqual(['y', 'z'])
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

  it('shows a deck under every group it sits in and in Ungrouped only when it sits in none', async () => {
    await renderPage([groupView('g1', 'Fun', ['d2', 'd3']), groupView('g2', 'Hardcore', ['d2'])])
    await screen.findByRole('list', { name: 'Decks in Fun' })
    expect(decksIn('Fun')).toEqual(['d2', 'd3'])
    expect(decksIn('Hardcore')).toEqual(['d2'])
    expect(decksIn('Ungrouped')).toEqual(['d1'])
    expect(screen.getAllByRole('link', { name: 'Krenko' })).toHaveLength(2)
    // The page counts decks, not places.
    expect(screen.getByText('3 decks')).toBeInTheDocument()
  })

  it('checks a deck into several groups with its Groups menu and out of one at a time', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun'), groupView('g2', 'Hardcore')])
    await screen.findByRole('button', { name: /^Fun/ })
    await user.click(screen.getByRole('button', { name: 'Groups of Krenko' }))
    const fun = await screen.findByRole('menuitemcheckbox', { name: 'Fun' })
    const hard = screen.getByRole('menuitemcheckbox', { name: 'Hardcore' })
    expect(fun).toHaveAttribute('aria-checked', 'false')
    expect(hard).toHaveAttribute('aria-checked', 'false')

    await user.click(fun)
    // The row moves from Ungrouped to Fun, taking its menu with it.
    await waitFor(() => expect(decksIn('Fun')).toEqual(['d2']))
    expect(fake.calls).toContain('add d2 g1')
    expect(decksIn('Ungrouped')).toEqual(['d1', 'd3'])

    await user.click(screen.getByRole('button', { name: 'Groups of Krenko' }))
    await user.click(await screen.findByRole('menuitemcheckbox', { name: 'Hardcore' }))
    await waitFor(() => expect(decksIn('Hardcore')).toEqual(['d2']))
    expect(fake.calls).toContain('add d2 g2')
    // The row stays in Fun, so its menu stays open with both groups checked.
    expect(screen.getByRole('menuitemcheckbox', { name: 'Fun' })).toHaveAttribute('aria-checked', 'true')
    expect(screen.getByRole('menuitemcheckbox', { name: 'Hardcore' })).toHaveAttribute('aria-checked', 'true')

    await user.click(screen.getByRole('menuitemcheckbox', { name: 'Fun' }))
    await waitFor(() => expect(screen.queryByRole('list', { name: 'Decks in Fun' })).toBeNull())
    expect(fake.calls).toContain('take out d2 g1')
    expect(decksIn('Hardcore')).toEqual(['d2'])
    expect(fake.groups.map((group) => group.decks)).toEqual([[], ['d2']])
    expect(screen.getByRole('status')).toHaveTextContent('Removed “Krenko” from Fun.')
  })

  it('makes a new group from a deck’s Groups menu and files the deck in it', async () => {
    const { fake, user } = await renderPage([groupView('g1', 'Fun', ['d2'])])
    await screen.findByRole('button', { name: /^Fun/ })
    await user.click(screen.getByRole('button', { name: 'Groups of Atraxa' }))
    await user.click(await screen.findByRole('menuitem', { name: 'New group…' }))
    expect(
      await screen.findByText('“Atraxa” joins the new group, and stays in its other groups.'),
    ).toBeInTheDocument()
    await user.type(screen.getByLabelText('Group name'), 'Dungeon')
    await user.click(screen.getByRole('button', { name: 'Create group' }))

    expect(await screen.findByRole('button', { name: /^Dungeon/ })).toBeInTheDocument()
    await waitFor(() => expect(decksIn('Dungeon')).toEqual(['d1']))
    expect(fake.calls).toEqual(expect.arrayContaining(['create Dungeon', 'add d1 new-1']))
    expect(fake.groups.map((group) => [group.name, group.decks])).toEqual([
      ['Fun', ['d2']],
      ['Dungeon', ['d1']],
    ])
    expect(screen.queryByRole('list', { name: 'Decks in Ungrouped' })?.textContent).not.toContain('Atraxa')
  })

  it('offers New group… in the deck menu before the account has any group', async () => {
    const { fake, user } = await renderPage()
    await screen.findByRole('list', { name: 'Your decks' })
    await user.click(await screen.findByRole('button', { name: 'Groups of Muldrotha' }))
    expect(screen.queryByRole('menuitemcheckbox')).toBeNull()
    await user.click(await screen.findByRole('menuitem', { name: 'New group…' }))
    await user.type(await screen.findByLabelText('Group name'), 'Fun')
    await user.click(screen.getByRole('button', { name: 'Create group' }))
    await waitFor(() => expect(decksIn('Fun')).toEqual(['d3']))
    expect(fake.calls).toEqual(expect.arrayContaining(['create Fun', 'add d3 new-1']))
  })

  it('shows a visible New group button in the page header', async () => {
    await renderPage([groupView('g1', 'Fun')])
    const button = await screen.findByRole('button', { name: 'New group' })
    // It sits in the header beside New deck and Import deck, not in a menu.
    expect(button.parentElement).toContainElement(screen.getByRole('button', { name: 'New deck' }))
    expect(button).toBeVisible()
  })

  describe('dragging a deck row', () => {
    const drop = (to: string, deckId: string, from: string, modifiers: { ctrlKey?: boolean } = {}) => {
      const data = new Map<string, string>([
        [DECK_DRAG_TYPE, deckId],
        [DECK_DRAG_FROM_TYPE, from],
      ])
      const dataTransfer = {
        types: [DECK_DRAG_TYPE, DECK_DRAG_FROM_TYPE],
        getData: (type: string) => data.get(type) ?? '',
        setData: () => undefined,
        dropEffect: 'none',
      }
      const section = screen.getByRole('button', { name: new RegExp(`^${to}`) }).closest('section')
      if (section === null) throw new Error('no section')
      // jsdom has no DragEvent, so the modifier keys are put on the events by hand.
      const fire = (make: typeof createEvent.drop) => {
        const event = make(section, { dataTransfer })
        Object.defineProperty(event, 'ctrlKey', { value: modifiers.ctrlKey ?? false })
        fireEvent(section, event)
      }
      act(() => {
        fire(createEvent.dragOver)
        fire(createEvent.drop)
      })
    }

    it('adds an ungrouped deck to the group it is dropped on', async () => {
      const { fake } = await renderPage([groupView('g1', 'Fun'), groupView('g2', 'Hardcore')])
      await screen.findByRole('button', { name: /^Fun/ })
      drop('Fun', 'd3', 'ungrouped')
      await waitFor(() => expect(decksIn('Fun')).toEqual(['d3']))
      expect(fake.calls).toEqual(['list', 'add d3 g1'])
      expect(screen.getByRole('status')).toHaveTextContent('Added “Muldrotha” to Fun.')
    })

    it('moves a deck out of the group it was dragged from, and keeps its other groups', async () => {
      const { fake } = await renderPage([
        groupView('g1', 'Fun', ['d2']),
        groupView('g2', 'Hardcore', ['d2']),
        groupView('g3', 'Dungeon'),
      ])
      await screen.findByRole('button', { name: /^Fun/ })
      drop('Dungeon', 'd2', 'g1')
      await waitFor(() => expect(decksIn('Dungeon')).toEqual(['d2']))
      expect(fake.groups.map((group) => group.decks)).toEqual([[], ['d2'], ['d2']])
      expect(fake.calls).toEqual(['list', 'add d2 g3', 'take out d2 g1'])
      expect(screen.getByRole('status')).toHaveTextContent('Moved “Krenko” from Fun to Dungeon.')
    })

    it('adds the deck and leaves it where it was while Ctrl is held', async () => {
      const { fake } = await renderPage([groupView('g1', 'Fun', ['d2']), groupView('g2', 'Hardcore')])
      await screen.findByRole('button', { name: /^Fun/ })
      drop('Hardcore', 'd2', 'g1', { ctrlKey: true })
      await waitFor(() => expect(decksIn('Hardcore')).toEqual(['d2']))
      expect(fake.groups.map((group) => group.decks)).toEqual([['d2'], ['d2']])
      expect(fake.calls).toEqual(['list', 'add d2 g2'])
    })

    it('takes the deck out of the group it was dragged from when it is dropped on Ungrouped', async () => {
      const { fake } = await renderPage([groupView('g1', 'Fun', ['d2']), groupView('g2', 'Hardcore', ['d2'])])
      await screen.findByRole('button', { name: /^Fun/ })
      drop('Ungrouped', 'd2', 'g1')
      await waitFor(() => expect(decksIn('Hardcore')).toEqual(['d2']))
      expect(screen.queryByRole('list', { name: 'Decks in Fun' })).toBeNull()
      expect(fake.groups.map((group) => group.decks)).toEqual([[], ['d2']])
      expect(fake.calls).toEqual(['list', 'take out d2 g1'])
      expect(screen.getByRole('status')).toHaveTextContent('Removed “Krenko” from Fun.')
    })

    it('does nothing for a drop on the group the deck came from', async () => {
      const { fake } = await renderPage([groupView('g1', 'Fun', ['d2'])])
      await screen.findByRole('button', { name: /^Fun/ })
      drop('Fun', 'd2', 'g1')
      expect(fake.calls).toEqual(['list'])
    })
  })

  it('shows one list without group controls when the groups cannot be had', async () => {
    await renderPage([groupView('g1', 'Fun')], 'network')
    expect(await screen.findByRole('list', { name: 'Your decks' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'New group' })).toBeNull()
    expect(screen.queryByRole('button', { name: /^Groups of / })).toBeNull()
  })
})
