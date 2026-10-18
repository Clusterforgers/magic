import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import type { ManaCost, ManaPool, ManaType, ObjectId } from '@/protocol/generated'
import { costPips } from '../cardModel'
import { LIFE_PER_SYMBOL } from '../manaSymbols'
import { ManaDot } from '../ManaPool'
import { ManaPips } from '../ManaPips'
import { manaName, spendWords } from './payText'
import { MANA_TYPES, manaOf, poolLeft } from './payment'
import type { LifeChoice, Spend } from './payment'

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

/** The life side of the summary: the life total, what paying would leave and a switch for each Phyrexian symbol. */
export interface LifeSummary {
  /** The payer's life total now. */
  total: number
  /** Life the payment pays so far. */
  paid: number
  /** One switch per symbol life can pay. */
  choices: readonly LifeChoice[]
  /** What the symbol can be paid with besides life, in words, for the switch's name: "white mana". */
  words(index: number): string
  onToggle(index: number): void
}

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
  /** Present when the cost has a symbol that life can pay. */
  life?: LifeSummary
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
  life,
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
      '[data-slot="pay-pool"], [data-slot="pay-auto"], [data-slot="pay-rest"], [data-slot="pay-cancel"]',
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
      {life === undefined ? null : <LifeRow life={life} cost={cost} />}
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
              variant="outline"
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

/** The Phyrexian symbols of the cost, each with a switch that pays 2 life for it instead of mana, and the life that leaves. */
function LifeRow({ life, cost }: { life: LifeSummary; cost: ManaCost }) {
  const pips = costPips(cost)
  const total = life.choices.reduce<Map<string, number>>((counts, choice) => {
    const symbol = pips[choice.index]?.symbol ?? ''
    return counts.set(symbol, (counts.get(symbol) ?? 0) + 1)
  }, new Map())
  const seen = new Map<string, number>()
  return (
    <Row label="Life">
      <Quiet>
        {life.paid === 0
          ? `You have ${life.total} life.`
          : `You have ${life.total} life, ${life.total - life.paid} after paying ${life.paid}.`}
      </Quiet>
      {life.choices.map((choice) => {
        const pip = pips[choice.index]
        const symbol = pip?.symbol ?? ''
        const nth = (seen.get(symbol) ?? 0) + 1
        seen.set(symbol, nth)
        // Equal symbols get a number, so their switches can be told apart.
        const several = total.get(symbol) ?? 1
        const which = several > 1 ? `, ${nth} of ${several}` : ''
        return (
          <Button
            key={choice.index}
            type="button"
            variant="outline"
            size="sm"
            data-slot="pay-life"
            // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
            aria-disabled={!choice.available}
            aria-pressed={choice.paid}
            aria-label={`Pay ${LIFE_PER_SYMBOL} life instead of ${life.words(choice.index)}${which}`}
            className="h-9 gap-1.5 px-2 aria-disabled:pointer-events-none aria-disabled:opacity-50 aria-pressed:bg-primary aria-pressed:text-primary-foreground"
            onClick={() => {
              if (choice.available) life.onToggle(choice.index)
            }}
          >
            {pip === undefined ? null : <ManaPips pips={[pip]} sizeClass={PIP_SIZE} label="Symbol" />}
            <span aria-hidden="true">
              {choice.paid ? `Paying ${LIFE_PER_SYMBOL} life` : `Pay ${LIFE_PER_SYMBOL} life`}
            </span>
          </Button>
        )
      })}
    </Row>
  )
}
