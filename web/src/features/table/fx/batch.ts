/** Finds the batches in the play store's changes and feeds what they cause to the decoration layer. */
import type { PlayState, PlayStore } from '@/play/store'
import type { EventView } from '@/protocol/generated'
import { deriveAttaches, deriveEffects } from './derive'
import type { Moment } from './derive'
import { markArrows, markEffects } from './marks'
import { deriveMoves } from './moves'
import type { Fx } from './store'

/** One applied batch: the game before it and after it, and its events. */
export interface BatchStep {
  before: Moment
  after: Moment
  events: readonly EventView[]
}

/**
 * The batch that turned `prev` into `next`, or null when the change was anything else. A batch moves `seq` on by
 * exactly the events it appends to the log, from where `prev` stood; a snapshot (a resync, a reconnect) is told apart by
 * `snapshots`, so its jump is never mistaken for play and never animates. A decision, a status change or a
 * chat line leaves the view as it was.
 */
export function batchBetween(prev: PlayState, next: PlayState): BatchStep | null {
  if (prev.view === null || next.view === null || next.view === prev.view) return null
  if (prev.seq === null || next.seq === null || next.seq <= prev.seq) return null
  // A snapshot may bring the log it spans (a reload, a gap), which would pass for the batch of the events it holds.
  if (prev.snapshots !== next.snapshots) return null
  const count = next.seq - prev.seq
  const entries = next.log.slice(-count)
  if (entries.length !== count || entries[0]?.seq !== prev.seq || entries.at(-1)?.seq !== next.seq - 1)
    return null
  return {
    before: { view: prev.view, objectsById: prev.objectsById },
    after: { view: next.view, objectsById: next.objectsById },
    events: entries.map((entry) => entry.event),
  }
}

/**
 * Plays the effects and starts the moves of every batch the store applies from now on; returns the function that stops
 * watching. This runs before React draws the batch, so a move's source is measured where the table still shows it.
 */
export function watchBatches(store: PlayStore, fx: Fx): () => void {
  return store.subscribe((next, prev) => {
    const step = batchBetween(prev, next)
    if (step === null) return
    const { moves, marks } = deriveMoves(step.before, step.after, step.events)
    fx.travel.launch(moves, step.after.view.viewer)
    // Measured now: the resolving object and what it affects may be gone from the table once React draws the batch.
    fx.effectArrows.play(markArrows(marks))
    fx.play([...deriveEffects(step.before, step.after, step.events), ...markEffects(marks)])
    fx.arrows.play(deriveAttaches(step.before, step.after))
  })
}
