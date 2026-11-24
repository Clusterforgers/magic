import { anchor } from '@/features/learn/anchors'
import { useEffect, useRef } from 'react'
import type { ReactNode } from 'react'
import { usePlay } from '@/features/game/gameContext'
import { cn } from '@/lib/utils'
import type { ManaOption, ManaType, ObjectId, ObjectView } from '@/protocol/generated'
import { costPips } from '../cardModel'
import { givesOfOptions, madeByOption, madeCost } from '../manaMade'
import type { Gives } from '../manaMade'
import { AnyPip, ManaPips, Pip } from '../ManaPips'
import { ArtDisc } from './detail/AskerChip'
import { lightHandlers } from './detail/boardBadges'
import { Kicker } from './detail/DecisionDetail'
import { optionKey } from './payment'
import type { Spend } from './payment'
import { paidCount } from './paySlots'
import type { PaySlot } from './paySlots'
import { manaName, spendWords } from './payText'
import { sourceMarks } from './sourceMarks'
import type { SourceMark } from './sourceMarks'

const SMALL = 'size-4 text-xs'

/** What stands under an open slot: the kind it asks for. */
function openCaption(slot: PaySlot, nameOf: (id: ObjectId) => string): string {
  if (slot.extraOf !== undefined) return `for ${nameOf(slot.extraOf)}`
  if (slot.accepts === null) return 'any'
  return slot.accepts.map(manaName).join(' or ')
}

/** What stands under a filled slot: the source that paid it. */
function paidCaption(slot: PaySlot, spends: readonly Spend[], nameOf: (id: ObjectId) => string): string {
  if (slot.fill?.life === true) return '2 life'
  const spend =
    slot.fill?.spend === null || slot.fill?.spend === undefined ? undefined : spends[slot.fill.spend]
  if (spend === undefined) return ''
  switch (spend.kind) {
    case 'pool':
    case 'restricted':
      return 'your pool'
    case 'tap':
      return nameOf(spend.option.source.object)
    case 'convoke':
      return nameOf(spend.creature)
    case 'improvise':
      return nameOf(spend.artifact)
    case 'delve':
      return nameOf(spend.card)
  }
}

/** The disc of a slot: the mana that paid it, else its symbol in a dashed ring. */
function SlotDisc({ slot, sizeClass }: { slot: PaySlot; sizeClass: string }) {
  const [symbol] = costPips([slot.symbol])
  const paid =
    slot.fill?.mana === null || slot.fill?.mana === undefined
      ? undefined
      : costPips(madeCost([slot.fill.mana]))[0]
  if (paid !== undefined) return <Pip pip={paid} sizeClass={sizeClass} />
  if (slot.fill === null && symbol !== undefined) {
    // Owed: the symbol as the cost prints it (a hybrid split, a Phyrexian phi), pale in a dashed ring.
    return (
      <span
        role="img"
        aria-label={`Owed: ${symbol.name}`}
        data-open=""
        className="inline-flex shrink-0 rounded-full outline-[1.5px] outline-offset-1 outline-border-strong outline-dashed [&>*]:opacity-45 [&>*]:ring-0"
      >
        <Pip pip={symbol} sizeClass={sizeClass} />
      </span>
    )
  }
  return (
    <span
      aria-hidden="true"
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-full bg-success font-bold text-surface',
        sizeClass,
      )}
    >
      ✓
    </span>
  )
}

/**
 * The payment's symbols as small discs for the band: filled with what paid them, dashed while owed, the own cost of a
 * source (a Signet's {1}) after a divider.
 */
export function PayMeter({ slots }: { slots: readonly PaySlot[] }) {
  return (
    <span
      data-slot="pay-meter"
      {...anchor('pay.meter')}
      role="img"
      aria-label={`${paidCount(slots)} of ${slots.length} paid`}
      className="inline-flex items-center gap-1"
    >
      {slots.map((slot, at) => (
        <span
          key={slot.key}
          data-paid={slot.fill !== null || undefined}
          className={cn(
            'inline-flex rounded-full',
            slot.fill !== null && 'ring-2 ring-success ring-offset-1 ring-offset-surface',
            slot.extraOf !== undefined &&
              slots[at - 1]?.extraOf === undefined &&
              'ml-1.5 border-l border-dashed border-border-strong pl-2 [border-radius:0]',
          )}
        >
          <SlotDisc slot={slot} sizeClass="size-6 text-xs" />
        </span>
      ))}
    </span>
  )
}

/** Permanents of one name that make the same mana: one row of the column. */
export interface MadeRow {
  name: string
  ids: ObjectId[]
}

/** Permanents that make the same mana in the same ways, a row for each name. */
export interface MadeGroup {
  key: string
  gives: Gives
  marks: readonly SourceMark[]
  rows: MadeRow[]
}

/** The sources grouped by what they make and then by name, in the order the options list them. */
export function groupByMade(
  sources: ReadonlyMap<ObjectId, readonly ManaOption[]>,
  nameOf: (id: ObjectId) => string,
): MadeGroup[] {
  const groups = new Map<string, MadeGroup>()
  for (const [id, options] of sources) {
    const gives = givesOfOptions(options)
    const marks = sourceMarks(options)
    const key = `${JSON.stringify(gives)}|${JSON.stringify(marks)}`
    const group = groups.get(key) ?? { key, gives, marks, rows: [] }
    groups.set(key, group)
    const name = nameOf(id)
    const row = group.rows.find((one) => one.name === name)
    if (row === undefined) group.rows.push({ name, ids: [id] })
    else row.ids.push(id)
  }
  return [...groups.values()]
}

/** The mana of one use, in words: "blue", "2 colorless", "white and blue". */
export function madeNames(made: readonly ManaType[]): string {
  const [first] = made
  if (first !== undefined && made.length > 1 && made.every((mana) => mana === first)) {
    return `${made.length} ${manaName(first)}`
  }
  return made.map(manaName).join(' and ')
}

/** What a group makes, in words: "white", "white or blue, you choose", "white and blue, costs 1 generic". */
export function groupWords(gives: Gives, marks: readonly SourceMark[]): string {
  const made = gives.any ? 'one mana of any color' : gives.ways.map((way) => madeNames(way.made)).join(' or ')
  const parts = [
    `Makes ${made}`,
    ...(gives.any || gives.ways.length > 1 ? ['you choose'] : []),
    ...(gives.cost.length > 0
      ? [
          `costs ${costPips(gives.cost)
            .map((pip) => pip.name)
            .join(' and ')}`,
        ]
      : []),
    ...(gives.sacrifice ? ['sacrifice'] : []),
    ...marks.map((mark) => mark.words),
  ]
  return parts.join(', ')
}

/** The heading of a group: "Makes {W} or {U} · you choose". */
function GroupLabel({ gives, marks }: { gives: Gives; marks: readonly SourceMark[] }) {
  return (
    <p
      data-slot="pay-group-label"
      aria-label={groupWords(gives, marks)}
      className="mt-1 flex flex-wrap items-center gap-1 text-xs leading-4 font-semibold text-muted-foreground"
    >
      <span aria-hidden="true">Makes</span>
      {gives.any ? (
        <>
          <AnyPip sizeClass={SMALL} />
          <span aria-hidden="true">any color</span>
        </>
      ) : (
        gives.ways.map((way, at) => (
          <span key={at} aria-hidden="true" className="inline-flex items-center gap-1">
            {at > 0 ? <span>or</span> : null}
            <ManaPips pips={costPips(madeCost(way.made))} sizeClass={SMALL} label="Mana" />
          </span>
        ))
      )}
      {gives.any || gives.ways.length > 1 ? <span aria-hidden="true">· you choose</span> : null}
      {gives.cost.length > 0 ? (
        <span aria-hidden="true" className="inline-flex items-center gap-1">
          · costs <ManaPips pips={costPips(gives.cost)} sizeClass={SMALL} label="Cost" />
        </span>
      ) : null}
      {gives.sacrifice ? <span aria-hidden="true">· sacrifice</span> : null}
      {marks.map((mark) => (
        <span key={mark.key} aria-hidden="true" data-mark={mark.key} title={mark.words}>
          · {mark.short}
        </span>
      ))}
    </p>
  )
}

const HALF =
  'inline-flex h-8 min-w-10 shrink-0 items-center justify-center gap-1 rounded-md border border-border-strong bg-surface px-1.5 text-xs font-semibold pointer-coarse:h-10 pointer-coarse:min-w-11 outline-offset-2 outline-primary hover:border-turn-you hover:bg-accent focus-visible:outline-2 aria-disabled:pointer-events-none aria-disabled:opacity-35'

/** The pips of what one option makes, on one line; `small` where a row holds many options. */
function MadePips({ option, small = false }: { option: ManaOption; small?: boolean }) {
  return (
    <ManaPips
      pips={costPips(madeCost(madeByOption(option)))}
      sizeClass={small ? SMALL : 'size-5 text-xs'}
      label="Mana"
      className="flex-nowrap"
    />
  )
}

/** Props of `PayColumn`. */
export interface PayColumnProps {
  /** The spell or ability paid for. */
  name: string
  /** Draws the name and the circles owed; off in a phone's sheet, whose bar holds the chip and the meter. */
  track?: boolean
  slots: readonly PaySlot[]
  spends: readonly Spend[]
  covered: boolean
  /** What the last change did, in a sentence; empty before any. */
  note: string
  nameOf(id: ObjectId): string
  /** The permanents that can be tapped for mana, each with its options. */
  sources: ReadonlyMap<ObjectId, readonly ManaOption[]>
  /** The options of a permanent that still help the payment. */
  fitting(id: ObjectId): readonly ManaOption[]
  /** The permanents the payment taps. */
  taken: readonly ObjectId[]
  /** The permanent clicked on the board that waits for its colour. */
  asking: ObjectId | undefined
  /** The other ways a tapped permanent could pay, without untapping it. */
  swaps: ReadonlyMap<ObjectId, readonly ManaOption[]>
  onTap(option: ManaOption): void
  onSwap(option: ManaOption): void
  /** The card that waits for its colour is let go, unanswered. */
  onCancelAsk(): void
  /** Takes the piece at this place of the payment back. */
  onRemove(index: number): void
  /** Takes the whole payment back. */
  onClear(): void
  /** The other ways to pay and what stands beside them: floating mana, convoke, life, abilities that activate at once. */
  children?: ReactNode
}

/**
 * A payment in the Stack column: one circle for each mana owed, filled with what paid it and captioned with its source
 * (a press takes it back; a Signet's own {1} stands after a divider), then every source as a row under what it makes,
 * with one button for each colour of a source that makes several and Untap on a row in use. A press on a card of the
 * board does the same, and a card that waits for its colour lights its row.
 */
export function PayColumn({
  name,
  track = true,
  slots,
  spends,
  covered,
  note,
  nameOf,
  sources,
  fitting,
  taken,
  asking,
  swaps,
  onTap,
  onSwap,
  onCancelAsk,
  onRemove,
  onClear,
  children,
}: PayColumnProps) {
  const objects = usePlay((state) => state.objectsById)
  const groups = groupByMade(sources, nameOf)
  const coachRow = groups
    .flatMap((group) => group.rows)
    .find((row) => row.ids.some((id) => !taken.includes(id)))
    ?.ids.join('-')
  const paid = paidCount(slots)
  // A piece taken back by its circle leaves the keyboard on the next one; with none left, on the band's answers.
  const root = useRef<HTMLDivElement>(null)
  const refocus = useRef<number | null>(null)
  useEffect(() => {
    const at = refocus.current
    if (at === null) return
    refocus.current = null
    const circles = [...(root.current?.querySelectorAll<HTMLElement>('button[data-slot="pay-circle"]') ?? [])]
    const next = circles.find((circle) => Number(circle.dataset['spend']) >= at) ?? circles.at(-1)
    const answers = document.querySelector<HTMLElement>(
      '[data-slot="decision-line"] [data-slot="pay-auto"]:not(:disabled), [data-slot="decision-line"] [data-slot="pay-rest"], [data-slot="decision-line"] [data-slot="undo"]',
    )
    ;(next ?? answers)?.focus()
  }, [spends])
  const take = (index: number): void => {
    refocus.current = index
    onRemove(index)
  }
  return (
    <div ref={root} data-slot="pay-column" className="grid min-w-0 gap-2">
      {track ? <Kicker lead="Paying" rest={`${name} · ${paid} of ${slots.length}`} /> : null}
      {track ? (
        <div
          data-slot="pay-track"
          role="group"
          aria-label="What is owed"
          className="flex flex-wrap gap-x-1 gap-y-1.5"
        >
          {slots.map((slot, at) => {
            const caption = slot.fill === null ? openCaption(slot, nameOf) : paidCaption(slot, spends, nameOf)
            const index = slot.fill?.spend ?? null
            const spend = index === null ? undefined : spends[index]
            const disc = <SlotDisc slot={slot} sizeClass="size-[2.125rem] text-[0.8125rem]" />
            return (
              <span
                key={slot.key}
                data-slot="pay-slot"
                data-paid={slot.fill !== null || undefined}
                data-extra={(slot.extraOf !== undefined && slots[at - 1]?.extraOf === undefined) || undefined}
                className="pay-slot"
              >
                {spend === undefined || index === null ? (
                  <span data-slot="pay-circle" className="!border-0">
                    {disc}
                  </span>
                ) : (
                  <button
                    type="button"
                    data-slot="pay-circle"
                    data-spend={index}
                    aria-label={`Remove ${spendWords(spend, nameOf)}`}
                    title={`Remove ${spendWords(spend, nameOf)}`}
                    className="!border-0 outline-offset-2 outline-primary focus-visible:outline-2"
                    onClick={() => take(index)}
                  >
                    {disc}
                  </button>
                )}
                <span data-slot="pay-caption" title={caption}>
                  {caption}
                </span>
              </span>
            )
          })}
        </div>
      ) : null}
      <p data-slot="pay-note" className="text-[0.8125rem] leading-[1.125rem] text-muted-foreground">
        {note !== ''
          ? note
          : covered
            ? 'Everything is paid. Confirm, or press a filled circle to take it back.'
            : 'Tap sources here or on your board.'}
        {spends.length === 0 ? null : (
          <button
            type="button"
            data-slot="pay-clear"
            aria-label="Clear the payment"
            className="ml-2 rounded-sm px-1 font-medium text-foreground underline-offset-2 outline-offset-2 outline-primary hover:underline focus-visible:outline-2"
            onClick={() => {
              refocus.current = 0
              onClear()
            }}
          >
            Clear
          </button>
        )}
      </p>
      {children}
      {/* The coach lights the Tap of the first row with a source still free (`pay.tap`). */}
      {groups.map((group) => (
        <div key={group.key} data-slot="pay-group" className="grid gap-1.5">
          <GroupLabel gives={group.gives} marks={group.marks} />
          <ul className="grid list-none gap-1.5">
            {group.rows.map((row) => {
              const used = row.ids.filter((id) => taken.includes(id))
              // The next one to tap: the one waiting for its colour, else the first that still helps.
              const free =
                row.ids.find((id) => id === asking) ??
                row.ids.find((id) => !taken.includes(id) && fitting(id).length > 0) ??
                row.ids.find((id) => !taken.includes(id))
              const last = used.at(-1)
              return (
                <SourceRow
                  key={row.ids.join('-')}
                  id={free ?? last ?? row.ids[0] ?? 0}
                  object={objects[free ?? last ?? row.ids[0] ?? 0]}
                  name={row.name}
                  count={row.ids.length}
                  usedCount={used.length}
                  options={free === undefined ? [] : (sources.get(free) ?? [])}
                  fits={free === undefined ? [] : fitting(free)}
                  paying={spends.flatMap((spend) =>
                    spend.kind === 'tap' && used.includes(spend.option.source.object)
                      ? madeByOption(spend.option)
                      : [],
                  )}
                  asking={asking !== undefined && row.ids.includes(asking)}
                  swaps={last === undefined ? [] : (swaps.get(last) ?? [])}
                  onTap={onTap}
                  onSwap={onSwap}
                  onCancelAsk={onCancelAsk}
                  coach={row.ids.join('-') === coachRow}
                  onUntap={() => {
                    const at = spends.findLastIndex(
                      (spend) => spend.kind === 'tap' && spend.option.source.object === last,
                    )
                    if (at >= 0) onRemove(at)
                  }}
                />
              )
            })}
          </ul>
        </div>
      ))}
    </div>
  )
}

/** One source: its picture and name, and a button for each way to tap it, or what it pays and Untap. */
function SourceRow({
  id,
  object,
  name,
  count,
  usedCount,
  options,
  fits,
  paying,
  asking,
  swaps,
  onTap,
  onSwap,
  onCancelAsk,
  onUntap,
  coach = false,
}: {
  /** Whether this row carries the coach's `pay.tap` anchor. */
  coach?: boolean
  id: ObjectId
  object: ObjectView | undefined
  name: string
  /** How many permanents of this name the row stands for, and how many of them the payment taps. */
  count: number
  usedCount: number
  /** The ways to tap the next one of them; none when every one is tapped. */
  options: readonly ManaOption[]
  fits: readonly ManaOption[]
  paying: readonly ManaType[]
  asking: boolean
  swaps: readonly ManaOption[]
  onTap(option: ManaOption): void
  onSwap(option: ManaOption): void
  onCancelAsk(): void
  onUntap(): void
}) {
  const usable = (option: ManaOption): boolean => fits.some((other) => optionKey(other) === optionKey(option))
  const several = options.length > 1
  const used = usedCount > 0
  const many = count > 1
  const whose = many ? `a ${name}` : name
  // A card pressed on the board waits for its colour here: the keyboard comes to the first kind that helps.
  const first = useRef<HTMLButtonElement>(null)
  const firstUsable = options.findIndex(usable)
  useEffect(() => {
    if (asking) first.current?.focus()
  }, [asking])
  return (
    <li
      role={asking ? 'group' : undefined}
      aria-label={asking ? `Mana from ${name}` : undefined}
      onKeyDown={
        asking
          ? (event) => {
              if (event.key !== 'Escape') return
              event.preventDefault()
              onCancelAsk()
            }
          : undefined
      }
      data-slot="pay-source-row"
      data-source={id}
      data-interactive=""
      data-used={(used && usedCount === count) || undefined}
      data-lit={asking || undefined}
      data-off={(!used && fits.length === 0) || undefined}
      className="detail-row min-h-[2.625rem] grid-cols-[auto_minmax(0,1fr)] py-1 pr-1"
      {...lightHandlers(id)}
    >
      <ArtDisc object={object} name={name} className="size-7 rounded-md" />
      {/* The name keeps a readable width; buttons that do not fit beside it wrap onto lines of their own under it. */}
      <span data-slot="pay-row-body" className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span className="grid min-w-0 flex-[1_1_6.5rem]">
          <span
            data-slot="row-name"
            title={name}
            className="line-clamp-2 text-[0.8125rem] leading-[1.125rem] font-semibold break-words"
          >
            {name}
            {many ? (
              <span data-slot="row-count" className="ml-1 font-normal text-muted-foreground tabular-nums">
                {used ? `${usedCount}/${count}` : `×${count}`}
              </span>
            ) : null}
          </span>
          {used ? (
            <span className="flex items-center gap-1 text-xs leading-4 font-semibold text-success">
              ✓ paying
              <ManaPips pips={costPips(madeCost(paying))} sizeClass="size-3.5 text-xs" label="Mana" />
            </span>
          ) : fits.length === 0 && options.length > 0 ? (
            // Why the row is off: its own cost is paid from mana made before it, or nothing owed takes its mana.
            <span data-slot="row-why" className="truncate text-xs leading-4 text-muted-foreground">
              {options.some((option) => (option.cost?.length ?? 0) > 0)
                ? 'Tap another source for its cost first'
                : 'Not needed for what is owed'}
            </span>
          ) : null}
        </span>
        <span
          data-slot="pay-row-options"
          className="ml-auto flex max-w-full min-w-0 flex-wrap items-center gap-1"
        >
          {options.map((option, at) => {
            const made = madeNames(madeByOption(option))
            return (
              <span key={optionKey(option)} className="flex shrink-0 items-center gap-1">
                {at > 0 && options.length <= 3 ? (
                  <span aria-hidden="true" className="text-xs text-muted-foreground">
                    or
                  </span>
                ) : null}
                <button
                  ref={at === firstUsable ? first : undefined}
                  {...(coach && at === Math.max(firstUsable, 0) ? anchor('pay.tap') : {})}
                  type="button"
                  data-slot="pay-tap"
                  // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                  aria-disabled={!usable(option)}
                  aria-label={
                    asking
                      ? `Add ${made} mana`
                      : `Tap ${whose} for ${made}${many ? `, ${usedCount} of ${count} used` : ''}`
                  }
                  title={`Tap for ${made}`}
                  className={cn(HALF, options.length > 3 && 'min-w-8 px-1')}
                  onClick={() => {
                    if (usable(option)) onTap(option)
                  }}
                >
                  {several ? <MadePips option={option} small={options.length > 3} /> : 'Tap'}
                </button>
              </span>
            )
          })}
          {asking ? (
            <button
              type="button"
              data-slot="pay-ask-cancel"
              aria-label="Cancel"
              title="Cancel"
              className={cn(HALF, 'min-w-8 px-1 pointer-coarse:min-w-11')}
              onClick={onCancelAsk}
            >
              <span aria-hidden="true">×</span>
            </button>
          ) : null}
          {used ? (
            <>
              {swaps.map((option) => (
                <button
                  key={optionKey(option)}
                  type="button"
                  data-slot="pay-switch"
                  aria-label={`Switch ${name} to ${madeNames(madeByOption(option))}`}
                  title={`Switch to ${madeNames(madeByOption(option))}`}
                  className={cn(HALF, swaps.length > 2 && 'min-w-8 px-1')}
                  onClick={() => onSwap(option)}
                >
                  <MadePips option={option} small={swaps.length > 2} />
                </button>
              ))}
              <button
                type="button"
                data-slot="pay-untap"
                aria-label={`Untap ${whose}`}
                className={HALF}
                onClick={onUntap}
              >
                Untap
              </button>
            </>
          ) : null}
        </span>
      </span>
    </li>
  )
}
