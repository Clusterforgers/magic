import { Switch as SwitchPrimitive } from '@base-ui/react/switch'
import { cn } from 'cn'

function Switch({ className, ...props }: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        // The ::after widens the press target past the 20 px track.
        'peer relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent transition-colors duration-fast ease-out after:absolute after:-inset-x-2 after:-inset-y-2 data-checked:bg-primary data-unchecked:bg-input data-disabled:cursor-not-allowed data-disabled:opacity-50',
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block size-4 rounded-full bg-surface ring-0 data-checked:translate-x-4 data-checked:bg-on-primary data-unchecked:translate-x-px motion-safe:transition-transform motion-safe:duration-fast motion-safe:ease-out"
      />
    </SwitchPrimitive.Root>
  )
}

export { Switch }
