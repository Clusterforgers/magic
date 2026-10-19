import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

/** A key cap: the name of a key as printed on it, for hints beside buttons and menu items. It takes the colour of its parent. */
export function Kbd({ className, ...props }: ComponentProps<'kbd'>) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        'inline-flex h-5 min-w-5 items-center justify-center rounded-sm border border-current/30 px-1 font-sans text-xs font-medium',
        className,
      )}
      {...props}
    />
  )
}
