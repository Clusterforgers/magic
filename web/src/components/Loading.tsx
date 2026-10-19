import type { ReactNode } from 'react'
import { useAfterDelay } from '@/hooks/useAfterDelay'

interface LoadingProps {
  /** What is loading, in words for a screen reader: "Loading the decks…". */
  label: string
  /** The skeleton: shapes the content will fill. Drawn only after the wait has lasted 300 ms. */
  children: ReactNode
  className?: string
}

/** A region that waits: a polite status for a screen reader at once and, if the wait drags on, a skeleton to look at. */
export function Loading({ label, children, className }: LoadingProps) {
  const drawn = useAfterDelay(true)
  return (
    <div role="status" aria-busy="true" data-slot="loading" className={className}>
      <span className="sr-only">{label}</span>
      {drawn ? children : null}
    </div>
  )
}
