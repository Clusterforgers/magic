import { useSyncExternalStore } from 'react'

/** How often the shared ticker fires, in milliseconds. */
export const TICK_MS = 1000

const listeners = new Set<() => void>()
let timer: ReturnType<typeof setInterval> | null = null
let now = 0

function subscribe(onTick: () => void): () => void {
  listeners.add(onTick)
  if (timer === null) {
    now = Date.now()
    timer = setInterval(() => {
      now = Date.now()
      for (const listener of listeners) listener()
    }, TICK_MS)
  }
  return () => {
    listeners.delete(onTick)
    if (listeners.size === 0 && timer !== null) {
      clearInterval(timer)
      timer = null
    }
  }
}

const idle = (): (() => void) => () => undefined

// Without a running timer the time is whole seconds, so two reads in one render agree.
const snapshot = (): number => (timer === null ? Math.floor(Date.now() / TICK_MS) * TICK_MS : now)

/**
 * The wall time (`Date.now()`), refreshed once a second while `on`: every caller shares one interval, which runs only
 * while some caller is on. Off, it does not re-render.
 */
export function useSecondTick(on: boolean): number {
  return useSyncExternalStore(on ? subscribe : idle, snapshot, snapshot)
}
