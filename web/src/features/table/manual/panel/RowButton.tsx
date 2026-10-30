import { ChevronRightIcon } from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { Ref } from 'react'
import { cn } from '@/lib/utils'

interface Props {
  icon: LucideIcon
  label: string
  /** The line under the name: what the command does, or why it cannot be used. */
  effect: string
  open: boolean
  /** The id of the form this row opens, named while it is open. */
  controls: string
  onToggle: () => void
  ref?: Ref<HTMLButtonElement>
}

/** The button of a panel row: the command's icon, its name and effect, and a chevron that turns down while the row is open. */
export function RowButton({ icon: Icon, label, effect, open, controls, onToggle, ref }: Props) {
  return (
    <button
      ref={ref}
      type="button"
      aria-expanded={open}
      aria-controls={open ? controls : undefined}
      onClick={onToggle}
      className="flex min-h-8 w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left outline-none select-none hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring pointer-coarse:min-h-11"
    >
      <Icon aria-hidden="true" data-slot="row-icon" className="size-4 shrink-0 text-muted-foreground" />
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="text-sm font-medium">{label}</span>
        <span className="text-xs text-muted-foreground">{effect}</span>
      </span>
      <ChevronRightIcon
        aria-hidden="true"
        data-slot="row-chevron"
        className={cn(
          'size-4 shrink-0 text-muted-foreground transition-transform duration-150 motion-reduce:transition-none',
          open && 'rotate-90',
        )}
      />
    </button>
  )
}
