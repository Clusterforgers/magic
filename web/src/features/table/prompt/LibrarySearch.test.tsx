/** Goal: a ChooseCards over library cards (a search) gets a search box from two cards that takes focus, matches every word in a card's name, type line or rules text, counts the cards in view, says when none matches, goes on to the cards with Arrow Down or Enter, clears with Escape before Escape undoes, and keeps a choice the search hides; a pick of a few cards elsewhere gets none. */
import { screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import type { CardDetail, ObjectId, ObjectView } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { setOracleFetcher } from './oracleWords'

const base = fixtureOf(2)
const field = base.view.objects.filter((o) => o.zone === 'Battlefield' && o.controller === 0).slice(0, 6)
const first = Math.max(...base.view.objects.map((o) => o.id)) + 1
/** Six cards of seat 0's library, copies of its permanents, each with a card of its own in the card database. */
const library: ObjectView[] = field.map((o, at) => ({
  ...o,
  id: first + at,
  zone: { Library: 0 },
  permanent: null,
  identity: { oracle_id: `card-${at}` },
}))
const ids = library.map((o) => o.id)
const fixture = { ...base, view: { ...base.view, objects: [...base.view.objects, ...library] } }
/** The rules text of the second card only. */
const RULES = 'When this enters, tap target quokka.'

afterEach(() => setOracleFetcher(() => Promise.reject(new Error('no server'))))

function mount(from: ObjectId[], max = 2, cancel = false) {
  setOracleFetcher((oracleId) =>
    Promise.resolve({ faces: [{ oracle_text: oracleId === 'card-1' ? RULES : '' }] } as CardDetail),
  )
  const table = renderDecision(
    {
      ChooseCards: {
        source: must(field[0]).id,
        from,
        min: 0,
        max,
        text: 'Search your library for up to two cards.',
        ...(cancel ? { can_cancel: true } : {}),
      },
    },
    { fixture },
  )
  const group = (): HTMLElement => screen.getByRole('group', { name: 'Cards to choose from' })
  const cards = (): HTMLElement[] => within(group()).queryAllByRole('button')
  const shown = (): ObjectId[] => cards().map((c) => Number(c.getAttribute('data-candidate')))
  const box = (): HTMLElement => screen.getByRole('searchbox', { name: 'Filter cards' })
  const count = (): string =>
    must(table.container.querySelector('[data-slot="filter-count"]')).textContent ?? ''
  return { ...table, cards, shown, box, count }
}

describe('a search of the library', () => {
  it('gets a search box that takes focus and counts every card', () => {
    const { box, count } = mount(ids.slice(0, 2))
    expect(box()).toHaveFocus()
    expect(count()).toBe('2 of 2')
  })

  /** The cards whose name or types hold every word, as the search box should keep them. */
  const holding = (words: string): ObjectId[] =>
    library
      .filter((o) => {
        const text = `${o.name} ${o.supertypes.join(' ')} ${o.types.join(' ')} ${o.subtypes.join(' ')}`
        return words
          .toLowerCase()
          .split(' ')
          .every((word) => text.toLowerCase().includes(word))
      })
      .map((o) => o.id)

  it.each([
    ['a name in capitals', must(library[3]).name.toUpperCase()],
    ['a type', 'creature'],
    ['a type and a name', `land ${must(library[0]).name.split(' ')[0] ?? ''}`],
  ])('narrows by %s, every word ignoring case', async (_, words) => {
    const { user, box, shown, count } = mount(ids)
    await user.type(box(), words)
    expect(shown()).toEqual(holding(words))
    expect(count()).toBe(`${holding(words).length} of ${ids.length}`)
  })

  it('matches the rules text once it is read', async () => {
    const { user, box, shown } = mount(ids)
    await user.type(box(), 'TAP quokka')
    await waitFor(() => expect(shown()).toEqual([ids[1]]))
  })

  it('says when no card matches, and Escape clears the words before it undoes', async () => {
    const { user, box, shown, container, answer } = mount(ids, 2, true)
    await user.type(box(), 'zzzz')
    expect(shown()).toEqual([])
    expect(must(container.querySelector('[data-slot="no-match"]')).textContent).toContain('zzzz')
    await user.keyboard('{Escape}')
    expect(box()).toHaveValue('')
    expect(shown()).toEqual(ids)
    expect(answer).not.toHaveBeenCalled()
  })

  it('goes on to the first card in view with Arrow Down and with Enter, which then chooses it', async () => {
    const { user, box, cards, answer } = mount(ids)
    await user.keyboard('{ArrowDown}')
    expect(cards()[0]).toHaveFocus()
    await user.click(box())
    await user.keyboard('{Enter}')
    expect(cards()[0]).toHaveFocus()
    await user.keyboard('{Enter}')
    expect(cards()[0]).toHaveAttribute('aria-pressed', 'true')
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer).toHaveBeenCalledWith({ ChooseCards: [ids[0]] })
  })

  it('keeps a card chosen while the search hides it, and says so', async () => {
    const { user, box, cards, count, answer } = mount(ids)
    await user.click(must(cards()[0]))
    await user.type(box(), 'zzzz')
    expect(count()).toBe(`0 of ${ids.length} · 1 chosen out of view`)
    await user.click(screen.getByRole('button', { name: 'Confirm' }))
    expect(answer).toHaveBeenCalledWith({ ChooseCards: [ids[0]] })
  })
})

describe('a pick of a few cards that are not in a library', () => {
  it('gets no search box', () => {
    renderDecision(
      { ChooseCards: { source: must(field[0]).id, from: field.map((o) => o.id), min: 0, max: 1 } },
      { fixture },
    )
    expect(screen.queryByRole('searchbox')).toBeNull()
  })
})
