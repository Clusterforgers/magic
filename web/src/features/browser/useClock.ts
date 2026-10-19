import { useEffect, useState } from 'react'

/** How often the ages are brought up to date, in milliseconds. */
export const CLOCK_MS = 30_000

/** The time in milliseconds, brought up to date every `intervalMs` while the tab is visible and the moment it is again. */
export function useClock(intervalMs: number): number {
  const [time, setTime] = useState(Date.now)
  useEffect(() => {
    const tick = (): void => setTime(Date.now())
    const timer = window.setInterval(() => {
      if (document.visibilityState !== 'hidden') tick()
    }, intervalMs)
    const shown = (): void => {
      if (document.visibilityState === 'visible') tick()
    }
    document.addEventListener('visibilitychange', shown)
    return () => {
      window.clearInterval(timer)
      document.removeEventListener('visibilitychange', shown)
    }
  }, [intervalMs])
  return time
}
