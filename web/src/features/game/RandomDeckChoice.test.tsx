/** Goal: the random deck buttons ask for the draw ids the server knows ("Random precon" for anyone, "Random from my decks" for a signed-in player, narrowed to a chosen group), the draw in force reads as pressed, nothing is sent while the lobby is busy, and the start's reveal lands in the chat as the table's own line for each seat that drew. */
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { isRandomId, RANDOM_MINE, RANDOM_PRECON, randomMineId } from './randomDeck'
import { RandomDeckChoice } from './RandomDeckChoice'

const GROUPS = [
  { id: 'g-1', name: 'Elves' },
  { id: 'g-2', name: 'Dragons' },
]

function mount(over: Partial<Parameters<typeof RandomDeckChoice>[0]> = {}) {
  const onChoose = vi.fn()
  render(
    <RandomDeckChoice
      signedIn
      groups={GROUPS}
      current={null}
      disabled={false}
      onChoose={onChoose}
      {...over}
    />,
  )
  return { onChoose, user: userEvent.setup() }
}

describe('the random deck buttons', () => {
  it.each([
    ['Random precon', [], null, { id: 'random:precon', name: 'Random precon' }],
    ['Random from my decks', [], null, { id: 'random:mine', name: 'Random deck' }],
    ['Random from my decks', GROUPS, 'Dragons', { id: 'random:mine:g-2', name: 'Random deck' }],
  ] as const)('%s with groups %o drawn from %s asks for %o', async (button, groups, group, pick) => {
    const { onChoose, user } = mount({ groups })
    if (group !== null) await user.selectOptions(screen.getByRole('combobox', { name: 'Draw from' }), group)
    await user.click(screen.getByRole('button', { name: button }))
    expect(onChoose).toHaveBeenCalledExactlyOnceWith(pick)
  })

  it('offers a guest only the precon draw, with no groups', () => {
    mount({ signedIn: false })
    expect(screen.getByRole('button', { name: 'Random precon' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Random from my decks' })).toBeNull()
    expect(screen.queryByRole('combobox')).toBeNull()
  })

  it('has no group list for a player without groups', () => {
    mount({ groups: [] })
    expect(screen.queryByRole('combobox')).toBeNull()
  })

  it.each([
    [RANDOM_PRECON, 'Random precon'],
    [RANDOM_MINE, 'Random from my decks'],
    [randomMineId('g-1'), 'Random from my decks'],
  ])('shows the draw %s in force as pressed', (current, button) => {
    mount({ current })
    expect(screen.getByRole('button', { name: button })).toHaveAttribute('aria-pressed', 'true')
  })

  it('sends nothing while the lobby is busy', async () => {
    const { onChoose, user } = mount({ disabled: true })
    await user.click(screen.getByRole('button', { name: 'Random precon' }))
    expect(onChoose).not.toHaveBeenCalled()
  })

  it.each([
    ['random:precon', true],
    ['random:mine:g-1', true],
    ['precon:c18-adaptive', false],
    [null, false],
  ])('tells %s is a draw: %s', (id, random) => {
    expect(isRandomId(id)).toBe(random)
  })
})

describe('the reveal at the start', () => {
  it('puts one table line per drawn deck in the chat', () => {
    const h = startSession()
    h.join()
    h.deliver(priorityDecision(5), {
      type: 'DecksRevealed',
      decks: [
        { seat: 0, name: 'Adaptive Enchantment' },
        { seat: 1, name: 'Elves' },
      ],
    })
    expect(h.state().chat).toEqual([
      { from: 0, text: 'drew Adaptive Enchantment', table: true },
      { from: 1, text: 'drew Elves', table: true },
    ])
  })
})
