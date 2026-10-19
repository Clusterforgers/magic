import { Dialog as SheetPrimitive } from "@base-ui/react/dialog"
import { XIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"

function Sheet(props: SheetPrimitive.Root.Props) {
  return <SheetPrimitive.Root data-slot="sheet" {...props} />
}

/**
 * A panel that hangs from the top bar of the page: it covers the page below the bar, the full width on a phone and
 * 24rem of it on a tablet. `top` is the height of the bar, a class such as `top-11`.
 */
function SheetContent({
  className,
  top = "top-11",
  children,
  ...props
}: SheetPrimitive.Popup.Props & { top?: string }) {
  return (
    <SheetPrimitive.Portal>
      <SheetPrimitive.Backdrop
        data-slot="sheet-overlay"
        className={cn(
          "fixed inset-x-0 bottom-0 z-50 bg-scrim transition-opacity duration-fast ease-out data-ending-style:opacity-0 data-starting-style:opacity-0",
          top
        )}
      />
      <SheetPrimitive.Popup
        data-slot="sheet-content"
        className={cn(
          "fixed inset-x-0 bottom-0 z-50 flex flex-col border-t bg-popover text-popover-foreground shadow-e3 outline-none popup-motion sm:inset-x-auto sm:right-0 sm:w-96 sm:border-l",
          top,
          className
        )}
        {...props}
      >
        {children}
        <SheetPrimitive.Close
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Close"
              className="absolute top-1 right-1"
            />
          }
        >
          <XIcon aria-hidden="true" />
        </SheetPrimitive.Close>
      </SheetPrimitive.Popup>
    </SheetPrimitive.Portal>
  )
}

function SheetTitle({ className, ...props }: SheetPrimitive.Title.Props) {
  return (
    <SheetPrimitive.Title
      data-slot="sheet-title"
      className={cn("sr-only", className)}
      {...props}
    />
  )
}

export { Sheet, SheetContent, SheetTitle }
