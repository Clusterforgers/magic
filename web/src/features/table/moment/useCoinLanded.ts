import { useEffect, useState } from 'react'
import { COIN_FLIP_MS } from './moments'

/** Whether a coin has landed: after `COIN_FLIP_MS`, or at once when `still`. */
export function useCoinLanded(still: boolean): boolean {
  const [landed, setLanded] = useState(still)
  useEffect(() => {
    if (still || landed) return undefined
    const timer = window.setTimeout(() => setLanded(true), COIN_FLIP_MS)
    return () => window.clearTimeout(timer)
  }, [still, landed])
  return still || landed
}
