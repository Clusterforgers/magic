// Goal: the create page is laid out for a wide window: the game (format and players) in one column and the options (visibility, house rules, advanced) in the other, in that reading order, with the Create button in a footer that sticks to the window's bottom and every other state of the page (loading, failed, no decks, game made) kept at the width of a page.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { chooseSeatLinks, openCreateForm } from '@/test-utils/createForm'
import { jsonResponse } from '@/test-utils/fakeFetch'
import { startServer } from '@/test-utils/fakeServer'
import { renderInRouter } from '@/test-utils/router'
import { CreateGamePage } from './CreateGamePage'

const column = (slot: 'game' | 'options') =>
  document.querySelector<HTMLElement>(`[data-slot="create-${slot}-column"]`)

const group = (name: string) => screen.getByRole('group', { name })
const house = () => document.querySelector<HTMLElement>('[data-slot="house-rules"]')
const advanced = () => screen.getByText('Advanced', { exact: true }).closest('details')

/** Whether `a` comes before `b` in the page, which is the order a person tabs and a screen reader reads. */
const before = (a: Element | null, b: Element | null): boolean =>
  a !== null && b !== null && Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING)

/** The width class of the box a page's content sits in, or null when it takes the width of the whole page. */
const pageBox = (inner: Element): Element | null => inner.closest('.max-w-page')

describe('the two columns', () => {
  it.each([
    ['commander', 'default'],
    ['normal', 'normal'],
  ] as const)('puts the format and the players in the game column in %s', async (_, format) => {
    await openCreateForm({}, format)
    const game = column('game')
    expect(game).toContainElement(group('Format'))
    expect(game).toContainElement(screen.getByRole('heading', { name: 'Players' }))
    expect(game).toContainElement(screen.getByLabelText('Seats'))
    expect(game).toContainElement(screen.getByLabelText('Your name'))
    expect(game).not.toContainElement(group('Visibility'))
  })

  it.each([
    ['commander', 'default'],
    ['normal', 'normal'],
  ] as const)(
    'puts visibility, the house rules and Advanced in the options column in %s',
    async (_, format) => {
      await openCreateForm({}, format)
      const options = column('options')
      expect(options).toContainElement(group('Visibility'))
      expect(options).toContainElement(house())
      expect(options).toContainElement(advanced())
      expect(options).not.toContainElement(group('Format'))
    },
  )

  it('keeps the deck of a Normal game with the creator’s name in the game column', async () => {
    await openCreateForm({}, 'normal')
    expect(column('game')).toContainElement(screen.getByLabelText('Your deck'))
  })

  it('reads in one order: format, players, visibility, house rules, Advanced, then the button', async () => {
    await openCreateForm()
    const order = [
      group('Format'),
      screen.getByRole('heading', { name: 'Players' }),
      group('Visibility'),
      house(),
      advanced(),
      screen.getByRole('button', { name: 'Create game' }),
    ]
    order.slice(1).forEach((element, at) => expect(before(order[at] ?? null, element)).toBe(true))
  })

  it('has the options column and the game column side by side from a medium window up, one above the other below', async () => {
    await openCreateForm()
    const columns = document.querySelector('[data-slot="create-columns"]')
    expect(columns).toHaveClass('md:grid-cols-2')
    expect(columns).not.toHaveClass('grid-cols-2')
  })

  it('moves the seat list to the game column and drops visibility with a link for every seat', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    expect(column('game')).toContainElement(screen.getByLabelText('Name for seat 1'))
    expect(column('game')).toContainElement(group('Format'))
    expect(column('options')).toContainElement(house())
    expect(column('options')).toContainElement(advanced())
    expect(screen.queryByRole('group', { name: 'Visibility' })).toBeNull()
  })
})

describe('the Create button', () => {
  it.each([
    ['the simple form', async () => (await openCreateForm()).user],
    ['the form of a Normal game', async () => (await openCreateForm({}, 'normal')).user],
    [
      'the form with a link for every seat',
      async () => {
        const { user } = await openCreateForm()
        await chooseSeatLinks(user)
        return user
      },
    ],
  ])('sits in a footer that sticks to the bottom of the window in %s', async (_, open) => {
    await open()
    const footer = document.querySelector<HTMLElement>('[data-slot="create-footer"]')
    expect(footer).toHaveClass('sticky', 'bottom-0')
    expect(footer).toContainElement(screen.getByRole('button', { name: 'Create game' }))
    expect(footer?.querySelectorAll('button')).toHaveLength(1)
  })

  it('comes after both columns and after what is wrong with the form', async () => {
    const { user } = await openCreateForm()
    await user.clear(screen.getByLabelText('Your name'))
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    const alert = await screen.findByRole('alert')
    const footer = document.querySelector('[data-slot="create-footer"]')
    expect(before(column('game'), footer)).toBe(true)
    expect(before(column('options'), footer)).toBe(true)
    expect(before(alert, footer)).toBe(true)
  })
})

describe('the width of the page', () => {
  it('gives the form the wide width, for its two columns', async () => {
    await openCreateForm()
    expect(document.querySelector('[data-slot="page"]')).toHaveAttribute('data-width', 'wide')
    expect(pageBox(screen.getByRole('form', { name: 'Create a game' }))).toBeNull()
  })

  it('keeps the links to a made game at the width of a page', async () => {
    const { user } = await openCreateForm()
    await chooseSeatLinks(user)
    await user.click(screen.getByRole('button', { name: 'Create game' }))
    const heading = await screen.findByRole('heading', { name: 'Game created' })
    expect(pageBox(heading)).not.toBeNull()
  })

  it.each([
    [
      'the decks fail',
      () => jsonResponse(503, { error: 'unavailable', message: 'down' }),
      'Cannot load the decks',
    ],
    ['there are no decks', () => jsonResponse(200, []), 'No decks to play with'],
  ])('keeps the alert at the width of a page when %s', async (_, decks, title) => {
    const server = startServer({ decks })
    await renderInRouter(<CreateGamePage api={server.api} />)
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(title)
    expect(pageBox(alert)).not.toBeNull()
  })
})
