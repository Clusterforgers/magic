/**
 * The strikes playing on one table: `launch` measures each striker and its target on the next frame (once React has
 * drawn the batch, a dying creature as its ghost) and hands `FxStrikes` what to draw. Nothing waits for it.
 */
import { createStore } from 'zustand/vanilla'
import type { StoreApi } from 'zustand/vanilla'
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { firstBox, STACK_ANCHOR } from './places'
import type { Box } from './places'
import type { FxMode } from './store'
import { STRIKE_MS } from './strikes'
import type { StrikePlan, StrikeTarget } from './strikes'
import { browserFrame } from './travel'
import type { FrameScheduler } from './travel'

/** One strike, measured and timed, ready to draw. */
export interface Striker {
  /** Unique to the batch and the source. */
  readonly key: string
  readonly source: ObjectId
  /** How the striker looks, drawn upright. */
  readonly look: ObjectView
  /** The picture's box where the striker stands. */
  readonly from: Box
  /** The box of what it strikes first. */
  readonly to: Box
  readonly mutual: boolean
  /** Milliseconds after the batch, stretched by the slow-down. */
  readonly delay: number
  /** Milliseconds, stretched by the slow-down. */
  readonly duration: number
}

/** What strikes now. Replaced, never changed. */
export interface StrikesState {
  readonly strikers: readonly Striker[]
}

/** What `createStrikes` can be given; tests replace the page and the frame. */
export interface StrikesOptions {
  /** What to look for cards and life totals in; the document by default. */
  root?: ParentNode
  /** Read at every batch: nothing strikes unless this says `full`. */
  mode: () => FxMode
  /** The next frame; the browser's by default. */
  frame?: FrameScheduler
  /** Multiplies every duration; 1 by default. */
  slow?: number
}

/** The strike layer of one table. */
export interface Strikes {
  readonly store: StoreApi<StrikesState>
  /** Starts the strikes of a batch; false when nothing will strike (another mode, no plan). */
  launch(plans: readonly StrikePlan[], look: (id: ObjectId) => ObjectView | undefined): boolean
  /** A strike is over: take it off the layer. */
  done(key: string): void
  /** Ends every strike now and drops what is waiting for its frame. */
  clear(): void
}

/** Where a permanent stands on the battlefield, or its ghost when it has just died; never its stack chip. */
export const sourceSelectors = (id: ObjectId): string[] => [
  `[data-object-id="${id}"]:not([data-fx-anchor="${STACK_ANCHOR}"] *)`,
  `[data-ghost="${id}"]`,
]

/** Where a strike lands: the permanent (or its ghost), or the seat's life total, else its panel or area. */
export function targetSelectors(target: StrikeTarget): string[] {
  if ('object' in target) return sourceSelectors(target.object)
  const seat: PlayerId = target.player
  return [
    `[data-seat="${seat}"] [data-slot="life"]`,
    `[data-tile-seat="${seat}"] [data-slot="life"]`,
    `[data-seat="${seat}"]`,
    `[data-tile-seat="${seat}"]`,
    `[data-seat-area="${seat}"]`,
  ]
}

/** The upright picture of a card in `box`: a tapped card's box is turned back to portrait about its centre. */
export function uprightBox(box: Box): Box {
  if (box.width <= box.height) return box
  return {
    x: box.x + (box.width - box.height) / 2,
    y: box.y - (box.width - box.height) / 2,
    width: box.height,
    height: box.width,
  }
}

/** A strike layer with nothing playing. */
export function createStrikes(options: StrikesOptions): Strikes {
  const frame = options.frame ?? browserFrame
  const slow = options.slow ?? 1
  const store = createStore<StrikesState>(() => ({ strikers: [] }))
  let cancelFrame: (() => void) | null = null
  let batches = 0

  function clear(): void {
    cancelFrame?.()
    cancelFrame = null
    if (store.getState().strikers.length > 0) store.setState({ strikers: [] })
  }

  function launch(plans: readonly StrikePlan[], look: (id: ObjectId) => ObjectView | undefined): boolean {
    if (plans.length === 0 || options.mode() !== 'full') return false
    clear()
    batches += 1
    const batch = batches
    cancelFrame = frame(() => {
      cancelFrame = null
      const root = options.root ?? document
      const strikers = plans.flatMap((plan): Striker[] => {
        const seen = look(plan.source)
        const first = plan.hits[0]
        const from = firstBox(root, sourceSelectors(plan.source))
        const to = first === undefined ? null : firstBox(root, targetSelectors(first.target))
        if (seen === undefined || from === null || to === null) return []
        return [
          {
            key: `${batch}:${plan.wave}:${plan.source}`,
            source: plan.source,
            look: seen,
            from: uprightBox(from),
            to,
            mutual: plan.mutual,
            delay: plan.wave * STRIKE_MS * slow,
            duration: STRIKE_MS * slow,
          },
        ]
      })
      if (strikers.length > 0) store.setState({ strikers })
    })
    return true
  }

  function done(key: string): void {
    const { strikers } = store.getState()
    const kept = strikers.filter((striker) => striker.key !== key)
    if (kept.length !== strikers.length) store.setState({ strikers: kept })
  }

  return { store, launch, done, clear }
}
