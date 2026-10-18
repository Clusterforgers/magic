import { Dialog as DialogPrimitive } from "@base-ui/react/dialog"
import { XIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"

function Dialog(props: DialogPrimitive.Root.Props) {
  return <DialogPrimitive.Root data-slot="dialog" {...props} />
}

function DialogContent({
  className,
  children,
  ...props
}: DialogPrimitive.Popup.Props) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Backdrop
        data-slot="dialog-overlay"
        className="fixed inset-0 z-50 bg-black/40 duration-100 data-ending-style:opacity-0 data-starting-style:opacity-0 motion-safe:transition-opacity"
      />
      <DialogPrimitive.Popup
        data-slot="dialog-content"
        className={cn(
          "fixed top-1/2 left-1/2 z-50 grid max-h-[85dvh] w-[calc(100vw-2rem)] max-w-3xl -translate-x-1/2 -translate-y-1/2 gap-3 overflow-y-auto rounded-xl bg-popover p-4 text-popover-foreground shadow-lg ring-1 ring-foreground/10 outline-none",
          className
        )}
        {...props}
      >
        {children}
        <DialogPrimitive.Close
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Close"
              className="absolute top-2 right-2"
            />
          }
        >
          <XIcon aria-hidden="true" />
        </DialogPrimitive.Close>
      </DialogPrimitive.Popup>
    </DialogPrimitive.Portal>
  )
}

function DialogTitle({ className, ...props }: DialogPrimitive.Title.Props) {
  return (
    <DialogPrimitive.Title
      data-slot="dialog-title"
      className={cn("pr-8 font-heading text-base font-medium", className)}
      {...props}
    />
  )
}

function DialogDescription({
  className,
  ...props
}: DialogPrimitive.Description.Props) {
  return (
    <DialogPrimitive.Description
      data-slot="dialog-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  )
}

export { Dialog, DialogContent, DialogDescription, DialogTitle }
