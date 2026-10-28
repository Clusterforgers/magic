import { act } from '@testing-library/react'
import { vi } from 'vitest'

/**
 * Fakes the timers and the date. Testing Library only waits through fake timers it finds in a global `jest`, so one
 * is set that advances Vitest's; without it every `findBy`, `waitFor` and user-event action hangs. `removeFakeClock`
 * undoes both.
 */
export function installFakeClock(): void {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] })
  Object.assign(globalThis, { jest: { advanceTimersByTime: (ms: number) => vi.advanceTimersByTime(ms) } })
}

/** Puts the real timers and the real date back. */
export function removeFakeClock(): void {
  vi.useRealTimers()
  Reflect.deleteProperty(globalThis, 'jest')
}

/**
 * Runs the timers still pending, then puts the real clock back. Dropping them would leave a library's teardown
 * (a menu that closes on a timer) undone for the next test; a polling page wants `removeFakeClock` instead.
 */
export async function flushFakeClock(): Promise<void> {
  await act(async () => {
    await vi.runOnlyPendingTimersAsync()
  })
  removeFakeClock()
}

/** Lets `ms` of fake time pass inside `act`, running every promise and timer that falls due on the way. */
export const settle = (ms = 0): Promise<void> =>
  act(async () => {
    await vi.advanceTimersByTimeAsync(ms)
  })
