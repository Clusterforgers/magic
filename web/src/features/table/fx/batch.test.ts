// Goal: of everything that changes the play store, exactly the batches the session applies are taken for play and hand their game before and after and their events to the decoration layer, while a snapshot (first, resync, repeated or from the past), a gap, a decision, a chat line, a lost connection and a batch that changes no view are never animated.
import { describe, expect, it } from 'vitest'
import { LOG_LIMIT } from '@/play/store'
import type { PlayState } from '@/play/store'
import type { EventView, StateView } from '@/protocol/generated'
import { FakeTimers } from '@/test-utils/fakes'
import { priorityDecision, startSession } from '@/test-utils/harness'
import { batchFrom, FOE, FOE_BEAR, ME, miniView, MY_GIANT, pub } from '@/test-utils/miniTable'
import type { Draft } from '@/test-utils/miniTable'
import { must } from '@/test-utils/batches'
import { batchBetween, watchBatches } from './batch'
import type { BatchStep } from './batch'
import { createFx } from './store'

const hurtGiant = (draft: Draft): void =>
  draft.edit(MY_GIANT, (object) => {
    must(object.permanent).damage = 2
  })
const PASS: EventView[] = pub({ PriorityPassed: { player: ME } })
const hitGiant: EventView[] = pub({
  DamageDealt: { source: FOE_BEAR, target: { Object: MY_GIANT }, amount: 2 },
})

/** A live session with a snapshot of the mini table at `seq`, and the result of `batchBetween` for every change after. */
function watch(seq = 4) {
  const h = startSession()
  h.join(ME, 2)
  h.deliver({ type: 'Snapshot', seq, state: miniView() })
  const seen: Array<BatchStep | null> = []
  h.session.store.subscribe((next, prev) => seen.push(batchBetween(prev, next)))
  const view = (): StateView => must(h.state().view)
  const send = (change: (draft: Draft) => void, events: EventView[] = PASS, from = h.state().seq): void =>
    h.deliver({ type: 'Events', ...batchFrom(view(), change, events, must(from)) })
  const steps = (): BatchStep[] => seen.filter((step): step is BatchStep => step !== null)
  return { h, seen, send, steps, view }
}

describe('the batches the session applies', () => {
  it('are found, with the game before, the game after and the events', () => {
    const { h, send, steps } = watch()
    const before = h.state()
    send(hurtGiant, hitGiant)
    const [step] = steps()
    expect(steps()).toHaveLength(1)
    expect(step?.before.view).toBe(before.view)
    expect(step?.before.objectsById).toBe(before.objectsById)
    expect(step?.after.view).toBe(h.state().view)
    expect(step?.after.objectsById).toBe(h.state().objectsById)
    expect(step?.events).toEqual(hitGiant)
    expect(must(step?.after.objectsById[MY_GIANT]?.permanent).damage).toBe(2)
  })

  it('come one after another, each starting from where the last ended', () => {
    const { send, steps } = watch()
    send(hurtGiant, hitGiant)
    send((draft) => draft.life(FOE, 17))
    send((draft) => draft.life(FOE, 15))
    const all = steps()
    expect(all).toHaveLength(3)
    expect(all[1]?.before.view).toBe(all[0]?.after.view)
    expect(all[2]?.before.view).toBe(all[1]?.after.view)
  })

  it('are not found without events, which a server never sends: the log does not move', () => {
    const { seen, send, steps } = watch()
    send((draft) => draft.life(FOE, 3), [])
    expect(seen).toHaveLength(1)
    expect(steps()).toEqual([])
  })

  it('are not found when the batch changes nothing the view holds, such as priority passing', () => {
    const { seen, send, steps } = watch()
    send(() => undefined)
    expect(seen).toHaveLength(1)
    expect(steps()).toEqual([])
  })

  it('are not found when they are too long for the log to hold, since their events are not all there', () => {
    const { send, steps } = watch()
    send(
      (draft) => draft.life(FOE, 1),
      Array.from(
        { length: LOG_LIMIT + 1 },
        () => ({ Public: { PriorityPassed: { player: ME } } }) satisfies EventView,
      ),
    )
    expect(steps()).toEqual([])
  })
})

describe('what is not a batch', () => {
  it('is the first view of a game, from a snapshot or from the batch at 0', () => {
    const h = startSession()
    h.join(ME, 2)
    const seen: Array<BatchStep | null> = []
    h.session.store.subscribe((next, prev) => seen.push(batchBetween(prev, next)))
    const first = { ...batchFrom(miniView(), () => undefined, PASS, 0), objects: miniView().objects }
    h.deliver({ type: 'Events', ...first })
    expect(h.state().view).not.toBeNull()
    expect(seen.length).toBeGreaterThan(0)
    expect(seen.every((step) => step === null)).toBe(true)
  })

  it('is a snapshot that jumps ahead after missed events (a resync), the next batch being found again', () => {
    const { h, seen, send, steps } = watch()
    const later = miniView()
    must(later.players[FOE]).life = 3
    h.deliver({ type: 'Snapshot', seq: 40, state: later })
    expect(seen.at(-1)).toBeNull()
    expect(steps()).toEqual([])
    send((draft) => draft.life(FOE, 1))
    expect(steps()).toHaveLength(1)
  })

  it.each([1, 3, 6])(
    'is a snapshot that brings the %i events it jumps over as its log, which are history and no batch',
    (missed) => {
      const { h, seen, send, steps } = watch()
      const later = miniView()
      must(later.players[FOE]).life = 3
      h.deliver({
        type: 'Snapshot',
        seq: 4 + missed,
        state: later,
        log: Array.from({ length: missed + 4 }, () => PASS[0] as EventView),
      })
      expect(must(h.state().log.at(-1)).seq).toBe(3 + missed)
      expect(seen.at(-1)).toBeNull()
      expect(steps()).toEqual([])
      send((draft) => draft.life(FOE, 1))
      expect(steps()).toHaveLength(1)
    },
  )

  it('is a snapshot of the same moment, which a plain Resync brings', () => {
    const { h, seen, steps } = watch()
    h.deliver({ type: 'Snapshot', seq: 4, state: miniView() })
    expect(seen.length).toBeGreaterThan(0)
    expect(steps()).toEqual([])
  })

  it('is a snapshot from before what the client already has', () => {
    const { h, send, steps } = watch(10)
    send((draft) => draft.life(FOE, 19))
    h.deliver({ type: 'Snapshot', seq: 6, state: miniView() })
    expect(steps()).toHaveLength(1)
    expect(must(h.state().seq)).toBe(6)
  })

  it('is a batch that does not follow on: the session asks for a resync and applies nothing', () => {
    const { h, seen, view } = watch()
    h.deliver({ type: 'Events', ...batchFrom(view(), hurtGiant, hitGiant, 99) })
    expect(seen).toEqual([])
    expect(h.sentTypes()).toContain('Resync')
  })

  it.each<[string, (h: ReturnType<typeof watch>['h']) => void]>([
    ['a decision arriving', (h) => h.deliver(priorityDecision(1))],
    ['a chat line', (h) => h.deliver({ type: 'Chat', from: FOE, text: 'good game' })],
    ['a lost connection', (h) => h.net.last.fail()],
  ])('is not %s', (_name, act) => {
    const { h, seen, steps } = watch()
    act(h)
    expect(seen.length).toBeGreaterThan(0)
    expect(steps()).toEqual([])
  })

  it('is any state that has no view yet, or no position in the log', () => {
    const state = (over: Partial<PlayState>): PlayState => ({ ...startSession().state(), ...over })
    const view = miniView()
    const some = state({ view, seq: 3 })
    expect(batchBetween(state({}), some)).toBeNull()
    expect(batchBetween(some, state({ view: null }))).toBeNull()
    expect(batchBetween(state({ view, seq: null }), some)).toBeNull()
    expect(batchBetween(some, state({ view: { ...view }, seq: null }))).toBeNull()
  })
})

describe('watching the store', () => {
  function watched() {
    const timers = new FakeTimers()
    const fx = createFx({ timers, mode: () => 'full' })
    const session = watch()
    const stop = watchBatches(session.h.session.store, fx)
    return { ...session, fx, stop, timers }
  }

  it('plays what each batch deserves', () => {
    const { send, fx } = watched()
    send(hurtGiant, hitGiant)
    expect(
      fx.store
        .getState()
        .objects.get(MY_GIANT)
        ?.map((entry) => entry.kind),
    ).toEqual(['damaged'])
    send((draft) => draft.life(FOE, 15))
    expect(
      fx.store
        .getState()
        .seats.get(FOE)
        ?.map((entry) => entry.amount),
    ).toEqual([5])
  })

  it('plays nothing for a snapshot, a decision or a chat line', () => {
    const { h, fx } = watched()
    h.deliver({
      type: 'Snapshot',
      seq: 20,
      state: { ...miniView(), players: miniView().players.map((p) => ({ ...p, life: 7 })) },
    })
    h.deliver(priorityDecision(1))
    h.deliver({ type: 'Chat', from: FOE, text: 'hi' })
    const state = fx.store.getState()
    expect(state.objects.size + state.seats.size + state.ghosts.size).toBe(0)
  })

  it('stops playing once it is told to stop', () => {
    const { send, fx, stop } = watched()
    stop()
    send(hurtGiant, hitGiant)
    expect(fx.store.getState().objects.size).toBe(0)
  })
})
