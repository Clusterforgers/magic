// Goal: the shared strip shows the spells on the stack as chips, the one that resolves first leading and ringed, as one tab stop moved with the arrow keys, each named with its caster for a screen reader, previewed on focus, and nothing at all while the stack is empty.
import { screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import { clone, must } from '@/test-utils/batches'
import { renderTable } from '@/test-utils/tableHarness'
import { StackChips } from './StackChips'

const chips = () => screen.getByRole('list', { name: 'Spells on the stack, top first' })
const emptyStack = (state: PlayState): PlayState => {
  const view = clone(must(state.view))
  view.stack = { count: 0, cards: [] }
  return { ...state, view }
}

describe('the chips', () => {
  it('lists the spells top first, each with its name and its cost', () => {
    renderTable({ seats: 4, ui: <StackChips /> })
    const items = within(chips()).getAllByRole('listitem')
    expect(items.map((item) => item.querySelector('span.truncate')?.textContent)).toEqual([
      'Lightning Bolt',
      'Giant Growth',
      'Counterspell',
    ])
    expect(items[0]?.querySelectorAll('[data-pip]')).toHaveLength(1)
    expect(items[2]?.querySelectorAll('[data-pip]')).toHaveLength(2)
  })

  it('names the top of the stack and the caster of each spell for a screen reader', () => {
    renderTable({ seats: 4, ui: <StackChips /> })
    expect(
      screen.getByRole('button', {
        name: /^Top of the stack: Lightning Bolt, cost red, Instant, cast by Alice$/,
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'Counterspell, cost blue, blue, Instant, cast by Cy' }),
    ).toBeInTheDocument()
  })

  it('rings the spell that resolves first', () => {
    renderTable({ seats: 4, ui: <StackChips /> })
    const [top, second] = within(chips()).getAllByRole('button')
    expect(top).toHaveClass('border-primary')
    expect(second).not.toHaveClass('border-primary')
  })

  it('is one tab stop that the arrow keys move along, in both directions', async () => {
    const { user } = renderTable({ seats: 4, ui: <StackChips /> })
    const buttons = within(chips()).getAllByRole('button')
    expect(buttons.map((button) => button.getAttribute('tabindex'))).toEqual(['0', '-1', '-1'])
    await user.tab()
    expect(buttons[0]).toHaveFocus()
    await user.keyboard('{ArrowRight}')
    expect(buttons[1]).toHaveFocus()
    await user.keyboard('{End}')
    expect(buttons[2]).toHaveFocus()
    await user.keyboard('{ArrowLeft}{Home}')
    expect(buttons[0]).toHaveFocus()
  })

  it('previews the card of a chip when it is focused', async () => {
    const { user } = renderTable({
      seats: 4,
      ui: (
        <>
          <StackChips />
          {null}
        </>
      ),
    })
    await user.tab()
    expect(document.activeElement).toHaveAttribute('data-object-id')
  })

  it('draws nothing while the stack is empty', () => {
    const { container } = renderTable({ seats: 4, ui: <StackChips />, state: emptyStack })
    expect(container.querySelector('[data-slot="stack-chips"]')).toBeNull()
    expect(screen.queryByRole('list', { name: 'Spells on the stack, top first' })).toBeNull()
  })

  it('puts a spell that is cast on the left and takes it away when it resolves', () => {
    const table = renderTable({ seats: 4, ui: <StackChips /> })
    table.update((state) => {
      const view = clone(must(state.view))
      view.stack = { count: 2, cards: must(view.stack.cards).slice(0, 2) }
      return { view }
    })
    expect(within(chips()).getAllByRole('button')).toHaveLength(2)
    table.update((state) => {
      const view = clone(must(state.view))
      view.stack = { count: 0, cards: [] }
      return { view }
    })
    expect(screen.queryByRole('list', { name: 'Spells on the stack, top first' })).toBeNull()
  })
})

describe('on the table', () => {
  it('sits in the shared strip, and the Stack tab holds the same spells with their casters', async () => {
    const { user, container } = renderTable({ seats: 4 })
    const strip = screen.getByRole('region', { name: 'Turn and priority' })
    expect(within(strip).getByRole('list', { name: 'Spells on the stack, top first' })).toBeInTheDocument()
    expect(container.querySelectorAll('[data-region="shared"] [data-slot="stack-chips"] li')).toHaveLength(3)
    await user.click(screen.getByRole('tab', { name: /^Stack/ }))
    expect(screen.getByRole('list', { name: 'Stack, top first' })).toBeInTheDocument()
  })
})
