/** A replay as plain data: the view and log at any step, folded from the steps with checkpoints, and the perspective. */
import { appendLog } from '@/play/store'
import type { LogEntry } from '@/play/store'
import { applyBatch, fromFirstBatch, indexById } from '@/play/view'
import type { EventBatch, PlayerId, Replay, StateView } from '@/protocol/generated'

/** The table at one step: the view and the log up to it. */
export interface Frame {
  view: StateView
  log: readonly LogEntry[]
  /** The log position after the step. */
  seq: number
}

/** How many steps apart the frames kept for going back are. */
export const CHECKPOINT_EVERY = 20

/** The frame after `step`, folded from `previous` (the frame before it). */
function next(previous: Frame, step: EventBatch): Frame {
  const view = applyBatch(previous.view, step)
  const log = appendLog(
    previous.log,
    step.from_seq,
    step.events,
    indexById(previous.view.objects),
    indexById(view.objects),
    step.shown,
  )
  return { view, log, seq: step.from_seq + step.events.length }
}

/**
 * Walks the steps of a replay: `frameAt(i)` folds from the nearest kept frame at or before step `i`, keeping one every
 * `CHECKPOINT_EVERY` steps, so stepping forward costs one fold and a jump back at most `CHECKPOINT_EVERY`. Null for a
 * replay without a first step at `from_seq` 0.
 */
export function createCursor(steps: readonly EventBatch[]): ((index: number) => Frame) | null {
  const first = steps[0]
  const start = first === undefined ? null : fromFirstBatch(first)
  if (first === undefined || start === null) return null
  const kept = new Map<number, Frame>([
    [0, { view: start, log: appendLog([], 0, first.events), seq: first.events.length }],
  ])
  let last: [number, Frame] = [0, kept.get(0) as Frame]
  return (wanted: number): Frame => {
    const index = Math.max(0, Math.min(steps.length - 1, Math.trunc(wanted)))
    let [at, frame] = last[0] <= index ? last : [0, kept.get(0) as Frame]
    if (at !== index) {
      for (let k = Math.floor(index / CHECKPOINT_EVERY) * CHECKPOINT_EVERY; k > at; k -= CHECKPOINT_EVERY) {
        const found = kept.get(k)
        if (found !== undefined) {
          ;[at, frame] = [k, found]
          break
        }
      }
    }
    while (at < index) {
      const step = steps[at + 1]
      if (step === undefined) break
      frame = next(frame, step)
      at += 1
      if (at % CHECKPOINT_EVERY === 0) kept.set(at, frame)
    }
    last = [at, frame]
    return frame
  }
}

/**
 * `view` laid out for `seat`: the table turns so that seat sits nearest, and it stays a spectator's view, so nothing can
 * be done on it. What the view holds is unchanged: a seat other than the viewer's own shows no hidden card.
 */
export function withPerspective(view: StateView, seat: PlayerId): StateView {
  return { ...view, viewer: seat, spectator: true }
}

/** The seat a replay opens at: the asking account's own, else the first. */
export const startSeat = (replay: Replay): PlayerId => replay.seat ?? 0
