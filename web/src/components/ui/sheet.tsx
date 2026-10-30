import { Dialog as SheetPrimitive } from "@base-ui/react/dialog"
import { XIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"

/** A non-modal dialog: the page around it stays pressable, and only Escape or Close shuts it. */
function Sheet(props: SheetPrimitive.Root.Props) {
  return <SheetPrimitive.Root data-slot="sheet" modal={false} disablePointerDismissal {...props} />
}

/**
 * A panel drawn into `container`, an overlay that is positioned and sized by the page: it fills the overlay's height
 * and the full width of it on a phone, 24rem from the right edge from `sm`. The overlay decides what the sheet leaves free.
 */
function SheetContent({
  className,
  container,
  children,
  ...props
}: SheetPrimitive.Popup.Props & { container: HTMLElement }) {
  return (
    <SheetPrimitive.Portal container={container}>
      <SheetPrimitive.Popup
        data-slot="sheet-content"
        className={cn(
          "pointer-events-auto absolute inset-y-0 right-0 z-50 flex w-full flex-col border-y bg-popover text-popover-foreground shadow-e3 outline-none popup-motion sm:w-96 sm:border-y-0 sm:border-l",
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
