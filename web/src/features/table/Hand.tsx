/** The most card backs drawn for a hand; the count says the rest. */
const MAX_BACKS = 7

/** A hand the viewer may not see: a fan of card backs. Decoration only; the pod's chip prints the count. */
export function HandBacks({ count }: { count: number }) {
  return (
    <span aria-hidden="true" data-slot="hand-backs" className="flex h-10 items-center pl-5">
      {Array.from({ length: Math.min(count, MAX_BACKS) }, (_, index) => (
        <span
          key={index}
          className="-ml-4 h-10 w-7 rounded-sm border border-primary/40 bg-linear-to-br from-primary/80 to-primary/40 ring-1 ring-foreground/20"
        />
      ))}
    </span>
  )
}
