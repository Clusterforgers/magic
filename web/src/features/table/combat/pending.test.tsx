// Goal: the choices a prompt has not sent yet live in one store per table: a change is heard once, "still nothing" is never a change, the store hands out the same object until something differs, and a prompt that publishes through the hook leaves its choices while it is on screen and takes them away when it goes, even when the choices change in between.
import { act, render, screen } from '@testing-library/react'
import { StrictMode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import type { AttackTarget, ObjectId } from '@/protocol/generated'
import {
  NO_PENDING,
  PendingCombatContext,
  createPendingCombatStore,
  usePendingCombat,
  usePublishPending,
} from './pending'
import type { PendingCombatStore } from './pending'

const blocks = (...pairs: [ObjectId, ObjectId][]): ReadonlyMap<ObjectId, ObjectId> => new Map(pairs)
const attacks = (...pairs: [ObjectId, AttackTarget][]): ReadonlyMap<ObjectId, AttackTarget> => new Map(pairs)

describe('the pending store', () => {
  it('starts with nothing chosen', () => {
    const store = createPendingCombatStore()
    expect(store.get()).toBe(NO_PENDING)
    expect(store.get().blocks.size + store.get().attacks.size).toBe(0)
  })

  it('holds what was set, and nothing else of it', () => {
    const store = createPendingCombatStore()
    const chosen = blocks([2, 1])
    store.set({ blocks: chosen })
    expect(store.get().blocks).toBe(chosen)
    expect(store.get().attacks.size).toBe(0)
    const aimed = attacks([5, { Player: 1 }])
    store.set({ attacks: aimed })
    expect(store.get().blocks.size).toBe(0)
    expect(store.get().attacks).toBe(aimed)
  })

  it('tells its listeners once per change', () => {
    const store = createPendingCombatStore()
    const listener = vi.fn()
    store.subscribe(listener)
    store.set({ blocks: blocks([2, 1]) })
    store.set({ blocks: blocks([2, 1], [3, 1]) })
    expect(listener).toHaveBeenCalledTimes(2)
  })

  it.each<[string, (store: PendingCombatStore) => void]>([
    ['setting nothing', (store) => store.set({})],
    ['clearing', (store) => store.clear()],
    ['setting empty maps', (store) => store.set({ blocks: blocks(), attacks: attacks() })],
  ])('hears nothing when %s changes nothing', (_name, act) => {
    const store = createPendingCombatStore()
    const listener = vi.fn()
    store.subscribe(listener)
    act(store)
    expect(listener).not.toHaveBeenCalled()
    expect(store.get()).toBe(NO_PENDING)
  })

  it('hears nothing when the very same maps are set again', () => {
    const store = createPendingCombatStore()
    const chosen = blocks([2, 1])
    store.set({ blocks: chosen })
    const listener = vi.fn()
    store.subscribe(listener)
    const before = store.get()
    store.set({ blocks: chosen })
    expect(listener).not.toHaveBeenCalled()
    expect(store.get()).toBe(before)
  })

  it('goes back to nothing, as one change, on clear and on emptied maps', () => {
    const store = createPendingCombatStore()
    store.set({ blocks: blocks([2, 1]) })
    const listener = vi.fn()
    store.subscribe(listener)
    store.clear()
    expect(store.get()).toBe(NO_PENDING)
    store.set({ attacks: attacks([4, { Player: 0 }]) })
    store.set({ attacks: attacks() })
    expect(store.get()).toBe(NO_PENDING)
    expect(listener).toHaveBeenCalledTimes(3)
  })

  it('stops telling a listener that unsubscribed, and lets one unsubscribe while it is being told', () => {
    const store = createPendingCombatStore()
    const first = vi.fn()
    const stop = store.subscribe(first)
    const second = vi.fn(() => stop())
    store.subscribe(second)
    store.set({ blocks: blocks([2, 1]) })
    store.set({ blocks: blocks([3, 1]) })
    expect(first).toHaveBeenCalledTimes(1)
    expect(second).toHaveBeenCalledTimes(2)
  })
})

/** A prompt that publishes what it is given. */
function Prompt({
  chosen,
  aimed,
}: {
  chosen?: ReadonlyMap<ObjectId, ObjectId>
  aimed?: ReadonlyMap<ObjectId, AttackTarget>
}) {
  usePublishPending({ blocks: chosen, attacks: aimed })
  return null
}

/** Shows what the store holds, as the overlay reads it. */
function Reader() {
  const { blocks: pairs, attacks: aims } = usePendingCombat()
  return <output>{`blocks ${[...pairs].join(';')} attacks ${[...aims.keys()].join(';')}`}</output>
}

function mount(ui: (store: PendingCombatStore) => React.ReactNode, strict = false) {
  const store = createPendingCombatStore()
  const tree = (node: React.ReactNode) => (
    <PendingCombatContext value={store}>
      {strict ? <StrictMode>{node}</StrictMode> : node}
    </PendingCombatContext>
  )
  const view = render(tree(ui(store)))
  return { store, view, again: (next: React.ReactNode) => view.rerender(tree(next)) }
}

describe('usePublishPending', () => {
  it('publishes the choices as the prompt mounts, before anything is painted', () => {
    const chosen = blocks([2, 1])
    const { store } = mount(() => (
      <>
        <Prompt chosen={chosen} />
        <Reader />
      </>
    ))
    expect(store.get().blocks).toBe(chosen)
    expect(screen.getByRole('status')).toHaveTextContent('blocks 2,1')
  })

  it('follows the choices as they change', () => {
    const { store, again } = mount(() => <Prompt chosen={blocks([2, 1])} />)
    const next = blocks([2, 1], [3, 4])
    again(<Prompt chosen={next} />)
    expect(store.get().blocks).toBe(next)
    const aimed = attacks([5, { Player: 2 }])
    again(<Prompt aimed={aimed} />)
    expect(store.get().blocks.size).toBe(0)
    expect(store.get().attacks).toBe(aimed)
  })

  it('does not publish again when a render brings the same maps', () => {
    const chosen = blocks([2, 1])
    const { store, again } = mount(() => <Prompt chosen={chosen} />)
    const listener = vi.fn()
    store.subscribe(listener)
    again(<Prompt chosen={chosen} />)
    expect(listener).not.toHaveBeenCalled()
  })

  it('takes the choices away when the prompt goes', () => {
    const { store, again } = mount(() => (
      <Prompt chosen={blocks([2, 1])} aimed={attacks([5, { Player: 0 }])} />
    ))
    again(null)
    expect(store.get()).toBe(NO_PENDING)
  })

  it('hands over to the next prompt: its choices replace the first ones', () => {
    const { store, again } = mount(() => <Prompt chosen={blocks([2, 1])} />)
    const aimed = attacks([5, { Player: 0 }])
    again(<Prompt key="other" aimed={aimed} />)
    expect(store.get().blocks.size).toBe(0)
    expect(store.get().attacks).toBe(aimed)
  })

  it('survives React running every effect twice in development', () => {
    const chosen = blocks([2, 1])
    const { store, again } = mount(() => <Prompt chosen={chosen} />, true)
    expect(store.get().blocks).toBe(chosen)
    again(null)
    expect(store.get()).toBe(NO_PENDING)
  })

  it('redraws a reader when the choices change, and only then', () => {
    const renders = vi.fn()
    function Counted() {
      renders()
      usePendingCombat()
      return null
    }
    const { store } = mount(() => <Counted />)
    expect(renders).toHaveBeenCalledTimes(1)
    const chosen = blocks([2, 1])
    act(() => store.set({ blocks: chosen }))
    expect(renders).toHaveBeenCalledTimes(2)
    act(() => store.set({ blocks: chosen }))
    expect(renders).toHaveBeenCalledTimes(2)
    act(() => store.clear())
    expect(renders).toHaveBeenCalledTimes(3)
    act(() => store.clear())
    expect(renders).toHaveBeenCalledTimes(3)
  })
})
