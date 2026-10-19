import { mergeProps } from '@base-ui/react/merge-props'
import { useRender } from '@base-ui/react/use-render'
import { cva } from 'class-variance-authority'
import type { VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

// Whole class names, so Tailwind can see them. Tints stay at 12 % so the text keeps its contrast (tokens.test.ts).
const pillVariants = cva(
  'inline-flex h-6 w-fit shrink-0 items-center justify-center gap-1 rounded-full border px-3 text-xs font-medium whitespace-nowrap transition-colors duration-fast ease-out data-selected:ring-2 data-selected:ring-ring [&>svg]:pointer-events-none [&>svg]:size-4 [&>svg]:shrink-0',
  {
    variants: {
      tone: {
        neutral: 'border-transparent bg-muted text-foreground data-interactive:hover:bg-accent',
        success: 'border-transparent bg-success/12 text-success data-interactive:hover:border-success',
        warning: 'border-transparent bg-warning/12 text-warning data-interactive:hover:border-warning',
        'turn-you':
          'border-transparent bg-turn-you text-on-turn-you data-interactive:hover:border-turn-you-edge',
        'turn-them': 'border-turn-them bg-transparent text-turn-them data-interactive:hover:bg-turn-them/12',
      },
    },
    defaultVariants: { tone: 'neutral' },
  },
)

/** The tones of a pill: neutral, success, warning, your turn and an opponent's turn. */
export type PillTone = NonNullable<VariantProps<typeof pillVariants>['tone']>

interface PillProps extends useRender.ComponentProps<'span'> {
  tone?: PillTone
  /** Marks a pill that is chosen, such as a filter that is on; the mark is a ring, not only a colour. */
  selected?: boolean
}

/**
 * A short status or label in a rounded capsule. Its words carry the meaning and the tone only underlines it. Pass a
 * button or link as `render` to make it a control; only then does it answer hover.
 */
export function Pill({ tone = 'neutral', selected = false, className, render, ...props }: PillProps) {
  return useRender({
    defaultTagName: 'span',
    props: mergeProps<'span'>({ className: cn(pillVariants({ tone }), className) }, props),
    render,
    state: {
      slot: 'pill',
      tone,
      selected: selected || undefined,
      interactive: render === undefined ? undefined : true,
    },
  })
}
