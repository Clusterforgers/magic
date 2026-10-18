import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import type { ManaCost, ManaPool, ManaType, ObjectId } from '@/protocol/generated'
import { costPips } from '../cardModel'
import { ManaDot } from '../ManaPool'
import { ManaPips } from '../ManaPips'
import { manaName } from './payText'
import { MANA_TYPES, manaOf, poolLeft } from './payment'
import type { Spend } from './payment'

const PIP_SIZE = 'size-5 text-xs'

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap items-center gap-x-3 gap-y-1">
      <span aria-hidden="true" className="w-24 shrink-0 text-xs text-muted-foreground">
        {label}
      </span>
      <div className="flex min-w-0 flex-wrap items-center gap-1.5">{children}</div>
    </div>
  )
}

const Quiet = ({ children }: { children: ReactNode }) => (
  <span className="text-xs text-muted-foreground">{children}</span>
)

/** Props of `PaymentSummary`. */
export interface PaymentSummaryProps {
  cost: ManaCost
  /** What the payment chosen so far leaves unpaid. */
  owed: ManaCost
  covered: boolean
  pool: ManaPool
  /** The payment so far, in order of choice. */
  spends: readonly Spend[]
  /** The name of a permanent. */
  nameOf(id: ObjectId): string
  /** True when one more of this floating mana would go toward the cost. */
  canSpend(mana: ManaType): boolean
  onSpendPool(mana: ManaType): void
  /** Takes the piece at this place in the payment back; Undo takes the last. */
  onRemove(index: number): void
  onClear(): void
}

/** What a piece of the payment is, in words: "green from Forest", "red from your pool". */
export function spendWords(spend: Spend, nameOf: (id: ObjectId) => string): string {
  const what = manaName(manaOf(spend))
  return spend.kind === 'pool'
    ? `${what} from your pool`
    : `${what} from ${nameOf(spend.option.source.object)}`
}

/**
 * What is being paid: the cost, the floating mana (each kind a button that spends one), the payment chosen so far (each
 * piece a button that takes it back, with Undo and Clear) and what is still owed. Taking something back by its button
 * keeps the keyboard where it was, since the button it was on goes away.
 */
export function PaymentSummary({
  cost,
  owed,
  covered,
  pool,
  spends,
  nameOf,
  canSpend,
  onSpendPool,
  onRemove,
  onClear,
}: PaymentSummaryProps) {
  const root = useRef<HTMLDivElement>(null)
  const refocus = useRef<number | null>(null)
  useEffect(() => {
    const at = refocus.current
    if (at === null) return
    refocus.current = null
    const chips = root.current?.querySelectorAll<HTMLElement>('[data-slot="pay-chip"]') ?? []
    // With nothing left to remove, the keyboard goes to the first control that adds, else to the actions.
    const fallback = root.current?.parentElement?.querySelector<HTMLElement>(
      '[data-slot="pay-pool"], [data-slot="pay-auto"], [data-slot="pay-cancel"]',
    )
    ;(chips[Math.min(at, chips.length - 1)] ?? fallback)?.focus()
  }, [spends])

  const take = (index: number): void => {
    refocus.current = index
    onRemove(index)
  }
  const held = MANA_TYPES.filter((type) => (pool.counts[MANA_TYPES.indexOf(type)] ?? 0) > 0)
  const seen = new Map<string, number>()
  const empty = spends.length === 0
  return (
    <div ref={root} data-slot="payment" className="grid w-full gap-1.5 text-sm">
      <Row label="Cost">
        <ManaPips pips={costPips(cost)} sizeClass={PIP_SIZE} label="Mana cost" />
      </Row>
      <Row label="Floating mana">
        {held.length === 0 ? (
          <Quiet>none</Quiet>
        ) : (
          held.map((type) => {
            const left = poolLeft(pool, spends, type)
            const usable = left > 0 && canSpend(type)
            return (
              <Button
                key={type}
                type="button"
                variant="outline"
                size="sm"
                data-slot="pay-pool"
                // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                aria-disabled={!usable}
                aria-label={`Spend ${manaName(type)} from your pool, ${left} left`}
                className="h-9 gap-1.5 px-2 aria-disabled:pointer-events-none aria-disabled:opacity-50"
                onClick={() => {
                  if (usable) onSpendPool(type)
                }}
              >
                <ManaDot type={type} />
                <span aria-hidden="true" className="tabular-nums">
                  {left}
                </span>
              </Button>
            )
          })
        )}
      </Row>
      <Row label="Paying with">
        {empty ? <Quiet>nothing yet</Quiet> : null}
        {spends.map((spend, index) => {
          const base = spend.kind === 'pool' ? `pool-${spend.mana}` : `tap-${spend.option.source.object}`
          const nth = seen.get(base) ?? 0
          seen.set(base, nth + 1)
          return (
            <Button
              key={`${base}-${nth}`}
              type="button"
              variant="secondary"
              size="sm"
              data-slot="pay-chip"
              aria-label={`Remove ${spendWords(spend, nameOf)}`}
              className="h-9 gap-1.5 px-2"
              onClick={() => take(index)}
            >
              <ManaDot type={manaOf(spend)} />
              <span aria-hidden="true">
                {spend.kind === 'pool' ? 'Pool' : nameOf(spend.option.source.object)}
              </span>
              <span aria-hidden="true" className="text-muted-foreground">
                ×
              </span>
            </Button>
          )
        })}
        {empty ? null : (
          <>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-9 px-2"
              aria-label="Undo the last choice"
              onClick={() => take(spends.length - 1)}
            >
              Undo
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-9 px-2"
              aria-label="Clear the payment"
              onClick={() => {
                refocus.current = 0
                onClear()
              }}
            >
              Clear
            </Button>
          </>
        )}
      </Row>
      <Row label="Still owed">
        {covered ? (
          <Quiet>nothing, the cost is covered</Quiet>
        ) : (
          <ManaPips pips={costPips(owed)} sizeClass={PIP_SIZE} label="Mana still owed" />
        )}
      </Row>
    </div>
  )
}
