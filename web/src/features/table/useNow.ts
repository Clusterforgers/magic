/** The time now for a request's clock, read again every second while it runs. */
import { useEffect, useState } from 'react'

/** The time now (`Date.now()`), read again every second while `running`. */
export function useNow(running: boolean): number {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (!running) return
    const timer = window.setInterval(() => setNow(Date.now()), 1000)
    return () => window.clearInterval(timer)
  }, [running])
  return now
}
