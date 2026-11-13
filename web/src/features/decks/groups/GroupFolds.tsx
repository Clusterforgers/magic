/** The heading of a collapsible group, which the deck pickers share (the lobby's deck list and the Delves picker). */
import { ChevronRightIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

/** The heading button of a group: chevron, name and count; it cannot close a group while a search shows them all. */
export function GroupToggle({
  name,
  count,
  open,
  controls,
  searching = false,
  onToggle,
}: {
  name: string
  count: number
  open: boolean
  /** The id of the group's list. */
  controls: string
  searching?: boolean
  onToggle(): void
}) {
  return (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={controls}
      disabled={searching}
      onClick={onToggle}
      className="flex min-h-8 w-full min-w-0 items-center gap-2 rounded-md px-1 text-left text-sm font-semibold hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:cursor-default disabled:hover:bg-transparent pointer-coarse:min-h-11"
    >
      <ChevronRightIcon
        aria-hidden="true"
        className={cn('size-4 shrink-0 transition-transform', open ? 'rotate-90' : '')}
      />
      <span className="min-w-0 truncate">{name}</span>
      <span className="shrink-0 text-xs font-normal text-muted-foreground">{count}</span>
    </button>
  )
}
