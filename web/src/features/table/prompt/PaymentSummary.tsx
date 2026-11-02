import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import type { ManaCost, ManaPool, ManaType, ObjectId } from '@/protocol/generated'
import { costPips } from '../cardModel'
import { madeCost } from '../manaMade'
import { keptOf } from '../manaKept'
import { LIFE_PER_SYMBOL } from '../manaSymbols'
import { ManaDot } from '../ManaPool'
import { ManaPips } from '../ManaPips'
import { manaName, spendWords } from './payText'
import { genericOf, MANA_TYPES, madeOf, poolLeft } from './payment'
import type { LifeChoice, Spend } from './payment'

const PIP_SIZE = 'size-5 text-xs'

/** One labelled line of the payment: a short label and its content; with `line` the content scrolls on one line. */
export function Row({
  label,
  children,
  line = false,
}: {
  label: string
  children: ReactNode
  line?: boolean
}) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap items-center gap-x-3 gap-y-1">
      <span aria-hidden="true" className="w-24 shrink-0 text-xs text-muted-foreground">
        {label}
      </span>
      <div
        data-line={line ? 'true' : undefined}
        className={
          line
            ? // `flex-1` and the contained width keep the line beside its label at the width the sheet has; it scrolls.
              'flex min-w-0 flex-1 flex-nowrap items-center gap-1.5 overflow-x-auto p-0.5 [contain:inline-size] [scrollbar-width:thin]'
            : 'flex min-w-0 flex-wrap items-center gap-1.5'
        }
      >
        {children}
      </div>
    </div>
  )
}

/** A labelled half of a shared line: the label in words beside its content. */
function Half({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={label} className="flex shrink-0 items-center gap-2">
      <span aria-hidden="true" className="text-xs text-muted-foreground">
        {label}
      </span>
      {children}
    </div>
  )
}

/** A muted word where a line has nothing to show. */
export const Quiet = ({ children }: { children: ReactNode }) => (
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

/** One restricted mana of the pool: what it is, what it may pay for and whether it can join the payment now. */
export interface RestrictedEntry {
  /** Its place in `ManaPool.restricted`. */
  index: number
  mana: ManaType
  /** What it may be spent on, in words. */
  words: string
  usable: boolean
}

/** What a permanent or card gives to the payment, in the order of the buttons: its name, then one button for each way. */
export interface Offer {
  key: string
  name: string
  choices: readonly OfferChoice[]
}

/** One way to pay with an offer. */
export interface OfferChoice {
  /** The text of the button when the offer has several ways. */
  label: string
  /** The name of the button for a screen reader. */
  aria: string
  /** The colour it pays, when it pays one. */
  mana?: ManaType
  spend: Spend
  usable: boolean
}

/** A labelled line of offers: "Convoke", "Improvise". */
export interface OfferRow {
  label: string
  offers: readonly Offer[]
}

/** The generic mana that improvise, delve and convoke for {1} pay, as one symbol. */
function genericMade(spends: readonly Spend[]): ManaCost {
  const total = spends.reduce((sum, spend) => sum + genericOf(spend), 0)
  return total > 0 ? [{ Generic: total }] : []
}

/** What keeps two chips of a payment apart. */
function chipKey(spend: Spend): string {
  if (spend.kind === 'pool') return `pool-${spend.mana}`
  if (spend.kind === 'restricted') return `restricted-${spend.index}`
  if (spend.kind === 'convoke') return `convoke-${spend.creature}`
  if (spend.kind === 'improvise') return `improvise-${spend.artifact}`
  if (spend.kind === 'delve') return `delve-${spend.card}`
  return `tap-${spend.option.source.object}`
}

/** What a chip is named after. */
function chipName(spend: Spend, nameOf: (id: ObjectId) => string): string {
  if (spend.kind === 'pool') return 'Pool'
  if (spend.kind === 'restricted') return 'Restricted'
  if (spend.kind === 'convoke') return `${nameOf(spend.creature)} (convoke)`
  if (spend.kind === 'improvise') return `${nameOf(spend.artifact)} (improvise)`
  if (spend.kind === 'delve') return `${nameOf(spend.card)} (delve)`
  return nameOf(spend.option.source.object)
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
  /** The restricted mana of the pool, each button spending one; present when the pool holds any. */
  restricted?: readonly RestrictedEntry[]
  onSpendRestricted?(index: number): void
  /** Permanents and cards that pay without making mana, one line for each way (convoke, improvise, delve). */
  offerRows?: readonly OfferRow[]
  onOffer?(spend: Spend): void
  /** Takes the piece at this place in the payment back; Undo takes the last. */
  onRemove(index: number): void
  onClear(): void
  /** Present when the cost has a symbol that life can pay. */
  life?: LifeSummary
  /** The row of mana sources, drawn between the floating mana and the payment. */
  sources?: ReactNode
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
  restricted = [],
  onSpendRestricted,
  offerRows = [],
  onOffer,
  onRemove,
  onClear,
  life,
  sources,
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
            const kept = keptOf(pool, type)
            return (
              <Button
                key={type}
                type="button"
                variant="outline"
                size="sm"
                data-slot="pay-pool"
                // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                aria-disabled={!usable}
                aria-label={`Spend ${manaName(type)} from your pool, ${left} left${kept === null ? '' : `, ${kept.words}`}`}
                title={kept?.words}
                className="h-9 gap-1.5 px-2 aria-disabled:pointer-events-none aria-disabled:opacity-50"
                onClick={() => {
                  if (usable) onSpendPool(type)
                }}
              >
                <ManaDot type={type} />
                <span aria-hidden="true" className="tabular-nums">
                  {left}
                </span>
                {kept === null ? null : (
                  <span
                    aria-hidden="true"
                    data-mark="kept"
                    className="rounded-sm bg-muted px-1 text-xs text-muted-foreground"
                  >
                    kept
                  </span>
                )}
              </Button>
            )
          })
        )}
      </Row>
      {restricted.length === 0 ? null : (
        <Row label="Restricted mana">
          {restricted.map((entry) => (
            <Button
              key={entry.index}
              type="button"
              variant="outline"
              size="sm"
              data-slot="pay-restricted"
              aria-disabled={!entry.usable}
              aria-label={`Spend restricted ${manaName(entry.mana)} from your pool, ${entry.words}`}
              className="h-9 gap-1.5 px-2 aria-disabled:pointer-events-none aria-disabled:opacity-50"
              onClick={() => {
                if (entry.usable) onSpendRestricted?.(entry.index)
              }}
            >
              <ManaDot type={entry.mana} />
              <span aria-hidden="true">{entry.words}</span>
            </Button>
          ))}
        </Row>
      )}
      {offerRows.map((row) => (
        <Row key={row.label} label={row.label}>
          {row.offers.map((offer) => (
            <span key={offer.key} className="flex items-center gap-1">
              {offer.choices.length === 1 ? null : (
                <span className="text-xs text-muted-foreground">{offer.name}</span>
              )}
              {offer.choices.map((choice) => (
                <Button
                  key={choice.aria}
                  type="button"
                  variant="outline"
                  size="sm"
                  data-slot="pay-offer"
                  aria-disabled={!choice.usable}
                  aria-label={choice.aria}
                  className="h-9 gap-1.5 px-2 aria-disabled:pointer-events-none aria-disabled:opacity-50"
                  onClick={() => {
                    if (choice.usable) onOffer?.(choice.spend)
                  }}
                >
                  {choice.mana === undefined ? null : <ManaDot type={choice.mana} />}
                  <span aria-hidden="true">{offer.choices.length === 1 ? offer.name : choice.label}</span>
                </Button>
              ))}
            </span>
          ))}
        </Row>
      ))}
      {life === undefined ? null : <LifeRow life={life} cost={cost} />}
      {sources}
      <Row label="Paying with">
        {empty ? <Quiet>nothing yet</Quiet> : null}
        {spends.map((spend, index) => {
          const base = chipKey(spend)
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
              <span aria-hidden="true" className="text-muted-foreground tabular-nums">
                {index + 1}.
              </span>
              {madeOf(spend).map((mana, at) => (
                <ManaDot key={`${mana}-${at}`} type={mana} />
              ))}
              {genericOf(spend) > 0 ? <span aria-hidden="true">{genericOf(spend)}</span> : null}
              <span aria-hidden="true">{chipName(spend, nameOf)}</span>
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
      <div data-slot="pay-made-owed" className="flex flex-wrap items-center gap-x-4 gap-y-1">
        <Half label="Made so far">
          {empty ? (
            <Quiet>nothing yet</Quiet>
          ) : (
            <ManaPips
              pips={costPips([...madeCost(spends.flatMap(madeOf)), ...genericMade(spends)])}
              sizeClass={PIP_SIZE}
              label="Mana made"
            />
          )}
        </Half>
        <Half label="Still owed">
          {covered ? (
            <Quiet>nothing, covered</Quiet>
          ) : (
            <ManaPips pips={costPips(owed)} sizeClass={PIP_SIZE} label="Mana still owed" />
          )}
        </Half>
      </div>
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
