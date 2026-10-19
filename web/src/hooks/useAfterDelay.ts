import { useEffect, useState } from 'react'

/** How long a wait must last before a skeleton is drawn for it; a quicker one would only flash. */
export const SKELETON_DELAY_MS = 300

/** True once `active` has been true for `ms` without a break; false again the moment it stops. */
export function useAfterDelay(active: boolean, ms: number = SKELETON_DELAY_MS): boolean {
  const [elapsed, setElapsed] = useState(false)
  useEffect(() => {
    if (!active) return
    const timer = window.setTimeout(() => setElapsed(true), ms)
    return () => {
      window.clearTimeout(timer)
      setElapsed(false)
    }
  }, [active, ms])
  return active && elapsed
}
