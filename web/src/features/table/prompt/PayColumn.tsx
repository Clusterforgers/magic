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
  const spend = slot.fill?.spend === null || slot.fill?.spend === undefined ? undefined : spends[slot.fill.spend]
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
  const paid = slot.fill?.mana === null || slot.fill?.mana === undefined ? undefined : costPips(madeCost([slot.fill.mana]))[0]
  if (paid !== undefined) return <Pip pip={paid} sizeClass={sizeClass} />
  return (
    <span
      aria-hidden="true"
      data-open={slot.fill === null || undefined}
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-full font-bold',
        sizeClass,
        slot.fill === null
          ? 'border-[1.5px] border-dashed border-border-strong text-muted-foreground'
          : 'bg-success text-surface',
      )}
    >
      {slot.fill === null ? (symbol?.text ?? '') : '✓'}
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

/** What a group makes, in words: "white", "white or blue, you choose", "white and blue, costs 1 generic". */
export function groupWords(gives: Gives, marks: readonly SourceMark[]): string {
  const made = gives.any
    ? 'one mana of any colour'
    : gives.ways.map((way) => way.made.map(manaName).join(' and ')).join(' or ')
  const parts = [
    `Makes ${made}`,
    ...(gives.any || gives.ways.length > 1 ? ['you choose'] : []),
    ...(gives.cost.length > 0 ? [`costs ${costPips(gives.cost).map((pip) => pip.name).join(' and ')}`] : []),
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
          <span aria-hidden="true">any colour</span>
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
  'inline-flex h-8 min-w-10 items-center justify-center gap-1 rounded-md border border-border-strong bg-surface px-1.5 text-xs font-semibold pointer-coarse:h-10 pointer-coarse:min-w-11 outline-offset-2 outline-primary hover:border-turn-you hover:bg-accent focus-visible:outline-2 aria-disabled:pointer-events-none aria-disabled:opacity-35'

/** The pips of what one option makes. */
function MadePips({ option }: { option: ManaOption }) {
  return <ManaPips pips={costPips(madeCost(madeByOption(option)))} sizeClass="size-5 text-xs" label="Mana" />
}

/** Props of `PayColumn`. */
export interface PayColumnProps {
  /** The spell or ability paid for. */
  name: string
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
  /** Takes the piece at this place of the payment back. */
  onRemove(index: number): void
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
  onRemove,
  children,
}: PayColumnProps) {
  const objects = usePlay((state) => state.objectsById)
  const groups = groupByMade(sources, nameOf)
  const paid = paidCount(slots)
  return (
    <div data-slot="pay-column" className="grid min-w-0 gap-2">
      <Kicker lead="Paying" rest={`${name} · ${paid} of ${slots.length}`} />
      <div data-slot="pay-track" role="group" aria-label="What is owed" className="flex flex-wrap gap-x-1 gap-y-1.5">
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
                  aria-label={`Take back ${spendWords(spend, nameOf)}`}
                  title={`Take back ${spendWords(spend, nameOf)}`}
                  className="!border-0 outline-offset-2 outline-primary focus-visible:outline-2"
                  onClick={() => onRemove(index)}
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
      <p data-slot="pay-note" className="text-[0.8125rem] leading-[1.125rem] text-muted-foreground">
        {note !== ''
          ? note
          : covered
            ? 'Everything is paid. Confirm, or press a filled circle to take it back.'
            : 'Tap sources here or on your board.'}
      </p>
      {children}
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
  onUntap,
}: {
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
  onUntap(): void
}) {
  const usable = (option: ManaOption): boolean => fits.some((other) => optionKey(other) === optionKey(option))
  const several = options.length > 1
  const used = usedCount > 0
  const many = count > 1
  const whose = many ? `a ${name}` : name
  return (
    <li
      data-slot="pay-source-row"
      data-source={id}
      data-interactive=""
      data-used={(used && usedCount === count) || undefined}
      data-lit={asking || undefined}
      data-off={(!used && fits.length === 0) || undefined}
      className="detail-row min-h-[2.625rem] grid-cols-[auto_minmax(0,1fr)_auto] py-1 pr-1"
      {...lightHandlers(id)}
    >
      <ArtDisc object={object} name={name} className="size-7 rounded-md" />
      <span className="grid min-w-0">
        <span className="truncate text-[0.8125rem] leading-[1.125rem] font-semibold">
          {name}
          {many ? (
            <span data-slot="row-count" className="ml-1 font-normal text-muted-foreground tabular-nums">
              {used ? `${usedCount}/${count}` : `×${count}`}
            </span>
          ) : null}
        </span>
        {used ? (
          <span className="flex items-center gap-1 text-[0.6875rem] leading-[0.875rem] font-semibold text-success">
            ✓ paying
            <ManaPips pips={costPips(madeCost(paying))} sizeClass="size-3.5 text-[0.625rem]" label="Mana" />
          </span>
        ) : fits.length === 0 && options.length > 0 ? (
          // Why the row is off: its own cost is paid from mana made before it, or nothing owed takes its mana.
          <span data-slot="row-why" className="truncate text-[0.6875rem] leading-[0.875rem] text-muted-foreground">
            {options.some((option) => (option.cost?.length ?? 0) > 0)
              ? 'Tap another source for its cost first'
              : 'Not needed for what is owed'}
          </span>
        ) : null}
      </span>
      <span className="flex items-center gap-1">
        {options.map((option, at) => {
          const made = madeByOption(option).map(manaName).join(' and ')
          return (
            <span key={optionKey(option)} className="flex items-center gap-1">
              {at > 0 && options.length <= 3 ? (
                <span aria-hidden="true" className="text-[0.6875rem] text-muted-foreground">
                  or
                </span>
              ) : null}
              <button
                type="button"
                data-slot="pay-tap"
                // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                aria-disabled={!usable(option)}
                aria-label={`Tap ${whose} for ${made}${many ? `, ${usedCount} of ${count} used` : ''}`}
                title={`Tap for ${made}`}
                className={cn(HALF, options.length > 3 && 'min-w-8 px-1')}
                onClick={() => {
                  if (usable(option)) onTap(option)
                }}
              >
                {several ? <MadePips option={option} /> : 'Tap'}
              </button>
            </span>
          )
        })}
        {used ? (
          <>
            {swaps.map((option) => (
              <button
                key={optionKey(option)}
                type="button"
                data-slot="pay-switch"
                aria-label={`Switch ${name} to ${madeByOption(option).map(manaName).join(' and ')}`}
                title={`Switch to ${madeByOption(option).map(manaName).join(' and ')}`}
                className={HALF}
                onClick={() => onSwap(option)}
              >
                <MadePips option={option} />
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
    </li>
  )
}
