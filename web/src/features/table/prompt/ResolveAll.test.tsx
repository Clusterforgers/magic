// Goal: "Resolve all" is one press in the dock exactly where the dock writes its full form and two or more objects wait on the stack, asks for the yield until the stack is empty once per prompt, and is never drawn over one object, an empty stack or in Full Control; the menu keeps the same yield in every form.
import { screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { Answer, FullControlMode, PriorityOptions, YieldUntil } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'

const OPTIONS: PriorityOptions = {
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  can_act: true,
  can_pass: true,
}

/** Seat 0 at priority on the opponent's turn with `objects` of the fixture's spells on the stack. */
function renderDock(width: number, objects: number, fullControl: FullControlMode = 'Off') {
  const answer = vi.fn((_answer: Answer) => true)
  const yieldUntil = vi.fn((_until: YieldUntil) => true)
  const table = renderTable({
    options: { viewport: { width, height: 900 } },
    session: { answer, yieldUntil },
    state: (state) => {
      const view = clone(must(state.view))
      view.turn = { ...view.turn, step: 'DeclareBlockers', active: 1 }
      const cards = must(view.stack.cards).slice(0, objects)
      view.stack = { count: cards.length, cards }
      return {
        ...state,
        view,
        fullControl,
        pending: { id: 4, decision: { player: 0, kind: { Priority: OPTIONS } } },
      }
    },
  })
  const button = (): HTMLElement | null => table.container.querySelector('[data-slot="resolve-all"]')
  const full = (): boolean => table.container.querySelector('[data-slot="key-cap"]') !== null
  return { ...table, answer, yieldUntil, button, full }
}

describe('Resolve all in the dock', () => {
  it.each([1920, 1600, 1280, 1024, 390])('at %i px is drawn exactly in the full form', (width) => {
    const { button, full } = renderDock(width, 3)
    expect(button() !== null).toBe(full())
  })

  it('has a width that draws it and a width that leaves it to the menu', async () => {
    expect(renderDock(1920, 3).button()).not.toBeNull()
    const phone = renderDock(390, 3)
    expect(phone.button()).toBeNull()
    await phone.user.click(screen.getAllByRole('button', { name: 'More ways to pass' }).at(-1) as HTMLElement)
    expect(
      await screen.findByRole('menuitem', { name: 'Resolve everything on the stack' }),
    ).toBeInTheDocument()
  })

  it.each([
    [0, false],
    [1, false],
    [2, true],
    [3, true],
  ])('over %i objects on the stack it is drawn: %s', (objects, drawn) => {
    expect(renderDock(1920, objects).button() !== null).toBe(drawn)
  })

  it.each<FullControlMode>(['NextWindow', 'Held'])('is not drawn in Full Control %s', (mode) => {
    expect(renderDock(1920, 3, mode).button()).toBeNull()
  })

  it('asks once for the yield until the stack is empty and never passes itself', async () => {
    const { button, user, yieldUntil, answer } = renderDock(1920, 3)
    const resolveAll = must(button())
    expect(resolveAll).toHaveTextContent('Resolve all')
    expect(resolveAll).toHaveAccessibleDescription(expect.stringContaining('until the stack is empty'))
    await user.click(resolveAll)
    await user.click(resolveAll)
    expect(yieldUntil.mock.calls).toEqual([['Resolve']])
    expect(answer).not.toHaveBeenCalled()
    expect(resolveAll).toBeDisabled()
  })
})
