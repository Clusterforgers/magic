/** The dashed frame around an Aura, Equipment or Fortification that sits beside its host, so the pair reads as one. */

/** Drawn inside the 6 px between boxes and the 4 px between lines; the host row keeps the attachment right behind it. */
export function AttachedMark() {
  return (
    <span
      aria-hidden="true"
      data-slot="attached-mark"
      className="pointer-events-none absolute -inset-0.5 rounded-lg border border-dashed border-foreground/70"
    />
  )
}
