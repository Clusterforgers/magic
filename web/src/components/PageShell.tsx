import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'
import type { PageWidth } from './pageWidths'

export type { PageWidth }

// Whole class names, so Tailwind can see them. The widths are the `--container-*` tokens of index.css.
const WIDTH: Record<PageWidth, string> = { narrow: 'max-w-narrow', page: 'max-w-page', wide: 'max-w-wide' }

interface PageShellProps extends ComponentProps<'div'> {
  width?: PageWidth
}

/** The column every page sits in: centred, one of the three widths, the same gutters. */
export function PageShell({ width = 'page', className, ...props }: PageShellProps) {
  return (
    <div
      data-slot="page"
      data-width={width}
      className={cn('mx-auto w-full px-4 py-6 sm:px-6 sm:py-8', WIDTH[width], className)}
      {...props}
    />
  )
}
