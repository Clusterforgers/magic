// Goal: the cursor of a replay gives at every step exactly the view a client folding the same batches holds, whether it walks forward, jumps back past checkpoints or jumps ahead, keeps the log in order, and a perspective only turns the table without changing what it shows.
import { describe, expect, it } from 'vitest'
import { applyBatch, fromFirstBatch } from '@/play/view'
import type { EventBatch, StateView } from '@/protocol/generated'
import { lobbyStart } from '@/test-utils/transcripts'
import { CHECKPOINT_EVERY, createCursor, withPerspective } from './replayModel'

const steps: EventBatch[] = lobbyStart.messages.flatMap((message) =>
  message.type === 'Events' ? [message] : [],
)

/** The view after step `index`, folded the plain way. */
function folded(index: number): StateView {
  let view = fromFirstBatch(steps[0] as EventBatch) as StateView
  for (const step of steps.slice(1, index + 1)) view = applyBatch(view, step)
  return view
}

describe('createCursor', () => {
  it('has a recording long enough to pass a checkpoint', () => {
    expect(steps[0]?.from_seq).toBe(0)
    expect(steps.length).toBeGreaterThan(CHECKPOINT_EVERY + 2)
  })

  it.each([
    ['forward', (n: number) => Array.from({ length: n }, (_, at) => at)],
    ['backward', (n: number) => Array.from({ length: n }, (_, at) => n - 1 - at)],
    ['by jumps', (n: number) => [n - 1, 0, Math.floor(n / 2), 1, n - 2, CHECKPOINT_EVERY + 1, 3]],
  ])('gives the folded view at every step walked %s', (_, order) => {
    const at = createCursor(steps)
    expect(at).not.toBeNull()
    for (const index of order(steps.length)) {
      const frame = at?.(index)
      expect(frame?.view).toEqual(folded(index))
      const seqs = frame?.log.map((entry) => entry.seq) ?? []
      expect(seqs).toEqual([...seqs].sort((a, b) => a - b))
      const step = steps[index] as EventBatch
      expect(frame?.seq).toBe(step.from_seq + step.events.length)
    }
  })

  it('clamps a step out of range', () => {
    const at = createCursor(steps)
    expect(at?.(-4).view).toEqual(folded(0))
    expect(at?.(steps.length + 9).view).toEqual(folded(steps.length - 1))
  })

  it('refuses steps that do not start at 0', () => {
    expect(createCursor([])).toBeNull()
    expect(createCursor(steps.slice(1))).toBeNull()
  })
})

describe('withPerspective', () => {
  it.each([0, 1])('turns the table to seat %i and changes nothing it shows', (seat) => {
    const view = folded(steps.length - 1)
    const turned = withPerspective(view, seat)
    expect(turned.viewer).toBe(seat)
    expect(turned.spectator).toBe(true)
    expect(turned.objects).toBe(view.objects)
    expect(turned.players).toBe(view.players)
  })
})
