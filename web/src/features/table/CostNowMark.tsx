import type { ManaCost } from '@/protocol/generated'
import { costPips } from './cardModel'
import { ManaPips } from './ManaPips'

/**
 * The cost a card's owner pays now where a rule drew its colors anew (the Delves uber boss), on the card's top edge over
 * the printed cost of its picture. It is decoration: the card's spoken name says the cost now and the printed one.
 */
export function CostNowMark({ cost }: { cost: ManaCost }) {
  return (
    <span
      aria-hidden="true"
      data-slot="cost-now"
      className="pointer-events-none absolute top-0.5 right-0.5 z-10 rounded-full bg-background/95 px-1 py-0.5 shadow-sm ring-2 ring-primary"
    >
      <ManaPips pips={costPips(cost)} sizeClass="size-4 text-xs" label="Costs now" />
    </span>
  )
}
