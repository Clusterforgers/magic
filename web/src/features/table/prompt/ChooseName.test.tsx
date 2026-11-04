/** Goal: the ChooseName prompt searches the whole catalog by what is typed (no list comes with the decision), offers at most eight hits and drops lands when the card says nonland, answers with the name of the hit clicked, Enter takes the exact or only hit, and "Name no card" answers with an empty name. */
import { screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { cardsApi } from '@/net/cards'
import type { CardHit, NameOptions } from '@/protocol/generated'
import { must } from '@/test-utils/batches'
import { creaturesOf, fixtureOf, renderDecision } from '@/test-utils/combatTable'
import { nameable } from './kinds/ChooseName'

const source = must(creaturesOf(fixtureOf(2), 0)[0])

const hit = (name: string, type_line = 'Creature — Elf'): CardHit =>
  ({ oracle_id: `id-${name}`, name, type_line }) as unknown as CardHit

function mount(hits: CardHit[], given: Partial<NameOptions> = {}) {
  const search = vi
    .spyOn(cardsApi, 'search')
    .mockResolvedValue({ cards: hits, total: hits.length, page: 1, per_page: 24 })
  const table = renderDecision({ ChooseName: { source, text: 'Choose a card name.', ...given } })
  return { ...table, search }
}

afterEach(() => vi.restoreAllMocks())

describe('the prompt', () => {
  it('searches what is typed and answers with the name of the hit clicked', async () => {
    const { user, answer, search } = mount([hit('Llanowar Elves'), hit('Elvish Mystic')])
    await user.type(screen.getByRole('searchbox'), 'elf')
    const button = await screen.findByRole('button', { name: 'Elvish Mystic' })
    expect(search).toHaveBeenLastCalledWith(expect.objectContaining({ text: 'elf' }), expect.anything())
    await user.click(button)
    expect(answer.mock.calls).toEqual([[{ ChooseName: 'Elvish Mystic' }]])
  })

  it('offers at most eight hits', async () => {
    const many = Array.from({ length: 20 }, (_, n) => hit(`Elf ${n}`))
    const { user } = mount(many)
    await user.type(screen.getByRole('searchbox'), 'elf')
    await screen.findByRole('button', { name: 'Elf 0' })
    expect(screen.getByRole('group', { name: 'Cards' }).querySelectorAll('button')).toHaveLength(8)
  })

  it('takes the only hit on Enter', async () => {
    const { user, answer } = mount([hit('Sol Ring', 'Artifact')])
    await user.type(screen.getByRole('searchbox'), 'sol')
    await screen.findByRole('button', { name: 'Sol Ring' })
    await user.keyboard('{Enter}')
    expect(answer.mock.calls).toEqual([[{ ChooseName: 'Sol Ring' }]])
  })

  it('names no card on request', async () => {
    const { user, answer } = mount([])
    await user.click(screen.getByRole('button', { name: 'Name no card' }))
    expect(answer.mock.calls).toEqual([[{ ChooseName: '' }]])
  })

  it('leaves lands out for a nonland name', async () => {
    const { user } = mount([hit('Forest', 'Basic Land — Forest'), hit('Llanowar Elves')], { nonland: true })
    await user.type(screen.getByRole('searchbox'), 'l')
    await screen.findByRole('button', { name: 'Llanowar Elves' })
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Forest' })).toBeNull())
  })
})

describe('nameable', () => {
  it.each([
    ['Basic Land — Forest', true, false],
    ['Land Creature — Forest Dryad', true, false],
    ['Creature — Elf', true, true],
    ['Basic Land — Forest', false, true],
    ['Artifact', true, true],
  ])('"%s" with nonland %s is %s', (type, nonland, kept) => {
    expect(nameable([hit('X', type)], nonland)).toHaveLength(kept ? 1 : 0)
  })
})
