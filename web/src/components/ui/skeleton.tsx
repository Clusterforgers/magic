import * as React from "react"
import { cn } from "cn"

// A grey block where content is on its way. It pulses where motion is welcome and stands still otherwise.
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      aria-hidden="true"
      className={cn("rounded-md bg-muted motion-safe:animate-pulse", className)}
      {...props}
    />
  )
}

export { Skeleton }
