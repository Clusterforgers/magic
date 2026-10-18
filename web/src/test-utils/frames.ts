/** Animation frames that run when the test says so, for code that takes a `Frames` (the combat overlay's scheduler). */
import type { Frames } from '@/features/table/combat/rects'

/** The frames, how many are waiting, and a way to run every waiting one. */
export interface FakeFrames {
  frames: Frames
  /** How many frames are asked for and not yet run or cancelled. */
  waiting(): number
  /** How many frames were ever asked for. */
  asked(): number
  /** Runs every waiting frame. */
  flush(): void
}

/** A fresh set of fake frames. */
export function fakeFrames(): FakeFrames {
  const queue = new Map<number, () => void>()
  let last = 0
  return {
    frames: {
      request(run) {
        last += 1
        queue.set(last, run)
        return last
      },
      cancel: (handle) => void queue.delete(handle),
    },
    waiting: () => queue.size,
    asked: () => last,
    flush() {
      const due = [...queue.values()]
      queue.clear()
      for (const run of due) run()
    },
  }
}
