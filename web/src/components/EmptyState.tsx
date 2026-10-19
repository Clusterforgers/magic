import type { LucideIcon } from 'lucide-react'
import type { ElementType, ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface EmptyStateProps {
  /** A line icon above the title; decoration, so it is hidden from a screen reader. */
  icon?: LucideIcon
  title: string
  /** The heading level of the title; `h2` under the page's own `h1`. */
  as?: 'h1' | 'h2' | 'h3'
  /** What to do about it, in a sentence. */
  children?: ReactNode
  /** The way out: buttons or links. */
  action?: ReactNode
  className?: string
}

/** What a list or page shows when there is nothing to list: why, and the next step. */
export function EmptyState({ icon: Icon, title, as = 'h2', children, action, className }: EmptyStateProps) {
  const Heading: ElementType = as
  return (
    <div
      data-slot="empty-state"
      className={cn('surface grid justify-items-center gap-3 border-dashed p-8 text-center', className)}
    >
      {Icon === undefined ? null : (
        <span className="flex size-10 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <Icon aria-hidden="true" className="size-5" />
        </span>
      )}
      <Heading className="text-lg font-semibold">{title}</Heading>
      {children === undefined ? null : (
        <p className="max-w-prose text-sm text-muted-foreground">{children}</p>
      )}
      {action === undefined ? null : <div className="flex flex-wrap justify-center gap-2">{action}</div>}
    </div>
  )
}
