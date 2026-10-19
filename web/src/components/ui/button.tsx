import { Button as ButtonPrimitive } from '@base-ui/react/button'
import { cva, type VariantProps } from 'class-variance-authority'
import { LoaderCircleIcon } from 'lucide-react'
import { cn } from 'cn'

// Heights are the target sizes of the design system: 32, 36 and 44 px, and 24 px for dense rows. Under a coarse
// pointer every control that people press often grows to 44 px.
const buttonStyles = cva(
  "group/button inline-flex shrink-0 items-center justify-center gap-2 rounded-lg border border-transparent text-sm font-medium whitespace-nowrap transition-colors duration-fast ease-out select-none motion-safe:active:not-aria-[haspopup]:translate-y-px aria-disabled:cursor-not-allowed aria-disabled:opacity-50 disabled:pointer-events-none disabled:opacity-50 aria-invalid:border-destructive [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: 'bg-primary text-on-primary hover:bg-primary/90',
        outline: 'border-border-strong bg-surface text-foreground hover:bg-accent aria-expanded:bg-accent',
        secondary: 'bg-secondary text-secondary-foreground hover:bg-accent aria-expanded:bg-accent',
        ghost: 'text-foreground hover:bg-accent aria-expanded:bg-accent',
        destructive: 'bg-destructive/10 text-destructive hover:bg-destructive/15',
        link: 'text-primary underline-offset-4 hover:underline',
      },
      size: {
        xs: 'h-6 gap-1 px-2 text-xs pointer-coarse:h-8',
        sm: 'h-8 px-3 pointer-coarse:h-11',
        default: 'h-9 px-4 pointer-coarse:h-11',
        lg: 'h-11 px-6 text-base',
        'icon-xs': 'size-6 pointer-coarse:size-8',
        'icon-sm': "size-8 pointer-coarse:size-11 [&_svg:not([class*='size-'])]:size-5",
        icon: "size-9 pointer-coarse:size-11 [&_svg:not([class*='size-'])]:size-5",
        'icon-lg': "size-11 [&_svg:not([class*='size-'])]:size-5",
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

/**
 * The classes of a button for a link or any other element. The variants are merged over the base, so a variant’s border
 * or text size wins, as it does on a `Button`.
 */
function buttonVariants(props?: Parameters<typeof buttonStyles>[0]): string {
  return cn(buttonStyles(props))
}

type ButtonProps = ButtonPrimitive.Props &
  VariantProps<typeof buttonStyles> & {
    /** Shows a spinner and ignores presses, but stays focusable so the busy state is not lost to a keyboard user. */
    loading?: boolean
  }

function Button({
  className,
  variant = 'default',
  size = 'default',
  loading = false,
  disabled,
  focusableWhenDisabled,
  children,
  ...props
}: ButtonProps) {
  return (
    <ButtonPrimitive
      data-slot="button"
      data-loading={loading ? '' : undefined}
      aria-busy={loading || undefined}
      disabled={disabled || loading}
      focusableWhenDisabled={focusableWhenDisabled || loading}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    >
      {loading ? <LoaderCircleIcon aria-hidden="true" className="motion-safe:animate-spin" /> : null}
      {children}
    </ButtonPrimitive>
  )
}

export { Button, buttonVariants }
export type { ButtonProps }
