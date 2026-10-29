import type { LucideIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

/** Props of `BarButton`: a `Button` with an icon and a short visible word. */
export interface BarButtonProps extends Omit<ComponentProps<typeof Button>, 'children' | 'size'> {
  icon: LucideIcon
  /** The accessible name and the tooltip: "Attack with all". */
  label: string
  /** The short word the eye reads beside the icon: "All"; hidden on a phone, where the icon stands for it. */
  word: string
  /** Keeps the word on a phone too (the button that answers). */
  keepWord?: boolean
}

/** A button of the combat bar: an icon and a short word, named in full for assistive technology and as a tooltip. */
export function BarButton({
  icon: Icon,
  label,
  word,
  keepWord = false,
  className,
  ...props
}: BarButtonProps) {
  return (
    <Button size="sm" aria-label={label} title={label} className={cn('gap-1.5 px-2.5', className)} {...props}>
      <Icon aria-hidden="true" className="size-4" />
      <span aria-hidden="true" className={keepWord ? undefined : 'max-sm:hidden'}>
        {word}
      </span>
    </Button>
  )
}
