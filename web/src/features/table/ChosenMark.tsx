import { Check } from 'lucide-react'

/** A chosen target that is a button (a player), filled in the chosen colour; the caller adds the check. */
export const CHOSEN_BUTTON =
  'border-selected bg-selected text-on-selected hover:bg-selected/90 hover:text-on-selected'

/**
 * The check on the corner of a chosen card, in the chosen colour beside its ring, so a choice never rests on colour
 * alone. It is decoration: the card says it is chosen with `aria-pressed` or its spoken name.
 */
export function ChosenMark() {
  return (
    <span
      aria-hidden="true"
      data-slot="chosen-mark"
      className="pointer-events-none absolute -top-1.5 -right-1.5 z-20 grid size-5 place-items-center rounded-full bg-selected text-on-selected ring-2 ring-background"
    >
      <Check className="size-3.5" strokeWidth={3} />
    </span>
  )
}
