// Goal: the top stack chip names what it aims at and the Stack tab lists the targets of its large top item, both naming them for a screen reader and marking an item with nothing legal left; the arrows run from the chip of the item on show to its targets and change with hover and focus, leave with the item, and cap at 6.
import { act, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PlayState } from '@/play/store'
import type { ObjectId, Target } from '@/protocol/generated'
import { clone, must } from '@/test-utils/batches'
import { boxOf, fakeRects } from '@/test-utils/rects'
import { renderTable } from '@/test-utils/tableHarness'
import { RectProviderContext } from './combat/rects'
import { GameTable } from './GameTable'
import { StackChips } from './StackChips'
import { StackPanel } from './StackPanel'

const on = (id: ObjectId): Target => ({ Object: { id, counter: 0 } })

/** The stack top first and some creatures on the battlefield, from the fixture. */
function facts(state: PlayState) {
  const view = must(state.view)
  const objects = must(view.objects)
  const stack = [...(view.stack.cards ?? [])].reverse()
  const creatures = objects
    .filter((o) => o.zone === 'Battlefield' && o.types.includes('Creature'))
    .map((o) => o.id)
  return { stack, creatures }
}

/** Gives stack item number `index` (top first) the target specifications `targets`. */
const aim =
  (...aimed: (Target[][] | undefined)[]) =>
  (state: PlayState): PlayState => {
    const view = clone(must(state.view))
    const { stack } = facts(state)
    aimed.forEach((targets, index) => {
      const object = view.objects.find((o) => o.id === stack[index])
      if (object !== undefined && targets !== undefined) object.targets = targets
    })
    return { ...state, view, objectsById: Object.fromEntries(view.objects.map((o) => [o.id, o])) }
  }

describe('the top chip', () => {
  it('names its targets after an arrow, beside the dot of its controller, and says them to a screen reader', () => {
    let creature = 0
    let name = ''
    renderTable({
      seats: 4,
      ui: <StackChips />,
      state: (state) => {
        creature = must(facts(state).creatures[0])
        name = must(must(state.objectsById)[creature]).name
        return aim([[on(creature), { Player: 1 }]])(state)
      },
    })
    const [top, second] = within(
      screen.getByRole('list', { name: 'Spells on the stack, top first' }),
    ).getAllByRole('button')
    expect(top?.querySelector('[data-slot="chip-targets"]')).toHaveTextContent(`→ ${name}, Bo`)
    expect(top?.querySelector('[data-slot="seat-dot"]')).not.toBeNull()
    expect(top).toHaveAccessibleName(
      `Top of the stack: Lightning Bolt, cost red, Instant, cast by Alice, targeting ${name} and Bo`,
    )
    expect(top).toHaveAttribute('data-stack-chip')
    expect(second?.querySelector('[data-slot="chip-targets"]')).toBeNull()
  })

  it('says "no legal target" and is dashed when everything it aimed at is gone', () => {
    renderTable({ seats: 4, ui: <StackChips />, state: aim([[on(4242)]]) })
    const top = within(screen.getByRole('list', { name: 'Spells on the stack, top first' })).getAllByRole(
      'button',
    )[0]
    expect(top).toHaveAttribute('data-no-target', 'true')
    expect(top).toHaveClass('border-dashed')
    expect(top).toHaveAccessibleName(/, no legal target left$/)
    expect(top).toHaveTextContent('→ no legal target')
  })

  it('names a target of a second item in its label but draws no line for it', () => {
    renderTable({
      seats: 4,
      ui: <StackChips />,
      state: (state) => aim(undefined, [[on(must(facts(state).creatures[0]))]])(state),
    })
    const second = within(screen.getByRole('list', { name: 'Spells on the stack, top first' })).getAllByRole(
      'button',
    )[1]
    expect(second).toHaveAccessibleName(/^Giant Growth, cost green, Instant, cast by Bo, targeting /)
    expect(second).not.toHaveTextContent('→')
  })
})

describe('the Stack tab', () => {
  it('shows the top item large with one row per target, a gone target struck out and spoken', () => {
    renderTable({
      seats: 4,
      ui: <StackPanel />,
      state: (state) => aim([[on(must(facts(state).creatures[0])), on(4242), { Player: 2 }]])(state),
    })
    const top = document.querySelector('[data-slot="stack-top"]')
    expect(top).not.toBeNull()
    const rows = within(top as HTMLElement)
      .getByRole('list', { name: 'Targets' })
      .querySelectorAll('li')
    expect(rows).toHaveLength(3)
    expect(rows[1]).toHaveAttribute('data-gone', 'true')
    expect(rows[1]).toHaveTextContent('no longer a legal target')
    expect(rows[2]).toHaveTextContent('Cy')
    expect(within(top as HTMLElement).getByRole('button')).toHaveAccessibleName(
      /, cast by Alice, targeting .* and Cy, 1 no longer legal$/,
    )
  })

  it('has no targets list for an item that aims at nothing and marks one with nothing left', () => {
    const { unmount } = renderTable({ seats: 4, ui: <StackPanel /> })
    expect(screen.queryByRole('list', { name: 'Targets' })).toBeNull()
    unmount()
    renderTable({ seats: 4, ui: <StackPanel />, state: aim([[on(4242)]]) })
    expect(document.querySelector('[data-slot="no-target"]')).toHaveTextContent('no legal target left')
  })
})

describe('the arrows', () => {
  const arrows = (): HTMLElement[] => [
    ...document.querySelectorAll<HTMLElement>('[data-layer="stack"] [data-line]'),
  ]

  function table(...aimed: (Target[][] | undefined)[]) {
    const rects = fakeRects((anchor) => boxOf(anchor))
    const harness = renderTable({
      seats: 4,
      state: (state) => aim(...aimed)(state),
      ui: (
        <RectProviderContext value={rects.provider}>
          <GameTable />
        </RectProviderContext>
      ),
    })
    return { ...harness, rects }
  }

  it('runs from the top chip to each target: cards and seats, in the colour of the viewer own item', () => {
    table([[on(1), on(2), { Player: 1 }]])
    expect(arrows().map((line) => [line.dataset.from, line.dataset.to])).toEqual([
      [
        'stack:' + must(document.querySelector('[data-stack-chip]')).getAttribute('data-stack-chip'),
        'card:1',
      ],
      [expect.any(String), 'card:2'],
      [expect.any(String), 'seat:1'],
    ])
    expect(arrows().every((line) => line.dataset.mine === 'true' && line.dataset.kind === 'target')).toBe(
      true,
    )
  })

  it('stops at 6 arrows however many targets there are', () => {
    table([Array.from({ length: 9 }, (_, index) => on(index + 1))])
    expect(arrows()).toHaveLength(6)
  })

  it('draws the arrows of another chip while it is focused and goes back to the top when focus leaves', async () => {
    const { user } = table([[on(1)]], [[on(2), on(3)]])
    expect(arrows().map((line) => line.dataset.to)).toEqual(['card:1'])
    const chips = within(screen.getByRole('list', { name: 'Spells on the stack, top first' })).getAllByRole(
      'button',
    )
    await act(async () => {
      chips[1]?.focus()
    })
    expect(arrows().map((line) => line.dataset.to)).toEqual(['card:2', 'card:3'])
    expect(arrows().every((line) => line.dataset.mine === undefined)).toBe(true)
    await user.tab()
    await act(async () => {
      chips[1]?.blur()
    })
    expect(arrows().map((line) => line.dataset.to)).toEqual(['card:1'])
  })

  it('leaves with the item: no arrows once the stack is empty or the item aims at nothing', () => {
    const { store } = table([[on(1)]])
    expect(arrows()).toHaveLength(1)
    act(() => {
      const view = clone(must(store.getState().view))
      view.stack = { count: 0, cards: [] }
      store.setState({ view })
    })
    expect(arrows()).toHaveLength(0)
    expect(document.querySelector('[data-layer="stack"]')).toBeNull()
  })

  it('draws no arrow to a target that is gone', () => {
    table([[on(4242), { Player: 2 }]])
    expect(arrows().map((line) => line.dataset.to)).toEqual(['seat:2'])
  })
})
