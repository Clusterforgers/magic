import { ArrowLeftRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { givesOfOptions } from '../manaMade'
import type { Gives } from '../manaMade'
import { ManaMade, madeWords } from '../ManaMade'
import { ManaChoice } from './ManaChoice'
import { Quiet, Row } from './PaymentSummary'
import { sourceMarks } from './sourceMarks'
import type { SourceMark } from './sourceMarks'

const PIP_SIZE = 'size-4 text-xs'

/** Permanents that give the same mana under the same name, which one chip stands for. */
export interface SourceGroup {
  name: string
  gives: Gives
  /** What a use takes or limits besides the mana: `only`, life, energy, other permanents. */
  marks: readonly SourceMark[]
  ids: readonly ObjectId[]
}

/** The mana sources grouped by name and by what they give, in the order the options list them. */
export function groupSources(
  sources: ReadonlyMap<ObjectId, readonly ManaOption[]>,
  nameOf: (id: ObjectId) => string,
): SourceGroup[] {
  const groups = new Map<string, { name: string; gives: Gives; marks: SourceMark[]; ids: ObjectId[] }>()
  for (const [id, options] of sources) {
    const name = nameOf(id)
    const gives = givesOfOptions(options)
    const marks = sourceMarks(options)
    const key = `${name}|${JSON.stringify(gives)}|${JSON.stringify(marks)}`
    const group = groups.get(key)
    if (group === undefined) groups.set(key, { name, gives, marks, ids: [id] })
    else group.ids.push(id)
  }
  return [...groups.values()]
}

/** The colour switch open on a chip: the permanent, the abilities it offers and whether it swaps a chosen one. */
export interface OpenSwitch {
  card: ObjectId
  options: readonly ManaOption[]
  swap: boolean
}

/** Props of `ManaSources`. */
export interface ManaSourcesProps {
  sources: ReadonlyMap<ObjectId, readonly ManaOption[]>
  nameOf(id: ObjectId): string
  /** The sources that still help the payment. */
  legal: readonly ObjectId[]
  /** The sources the payment uses. */
  taken: readonly ObjectId[]
  /** What a click on the source does on its card: adds it, asks which mana, or takes it back. */
  onAct(id: ObjectId): void
  /** The colour switch drawn beside its chip. */
  open?: OpenSwitch
  onPick?(option: ManaOption): void
  onClose?(): void
  /** Chosen permanents whose colour can change without taking them back. */
  swappable?: readonly ObjectId[]
  onSwitch?(id: ObjectId): void
}

/**
 * The permanents the payment can tap on one scrolling line, one chip each (equal ones share a chip with a count) with
 * the symbols of what a tap gives: `{C}{C}` for a Sol Ring, `{1}: {W}{B}` for a Signet, a five-colour disc for a source
 * of any colour, then its marks (`only`, life, energy, other permanents). A click does what a click on the card does; a
 * chip is pressed while the payment uses one of its permanents. The colour switch opens beside the chip, and a chosen
 * source of several colours has a switch button that changes its colour without taking it back.
 */
export function ManaSources({
  sources,
  nameOf,
  legal,
  taken,
  onAct,
  open,
  onPick,
  onClose,
  swappable = [],
  onSwitch,
}: ManaSourcesProps) {
  const groups = groupSources(sources, nameOf)
  return (
    <Row label="Mana sources" line>
      {groups.length === 0 ? <Quiet>none</Quiet> : null}
      {groups.map((group) => {
        const used = group.ids.filter((id) => taken.includes(id))
        const free = group.ids.find((id) => legal.includes(id) && !taken.includes(id))
        const target = free ?? used.at(-1)
        const many = group.ids.length > 1
        const pressed = used.length > 0
        const swapped = used.findLast((id) => swappable.includes(id))
        const here = open !== undefined && group.ids.includes(open.card) ? open : undefined
        const switchable = swapped !== undefined && here === undefined
        const marked = group.marks.map((mark) => `, ${mark.words}`).join('')
        return (
          <span
            key={group.ids.join('-')}
            data-slot="pay-source-group"
            className="flex shrink-0 items-center gap-1"
          >
            <span className="flex items-center">
              <Button
                type="button"
                variant="outline"
                size="sm"
                data-slot="pay-source"
                // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
                aria-disabled={target === undefined}
                aria-pressed={pressed}
                aria-label={`Tap ${many ? `a ${group.name}` : group.name} for ${madeWords(group.gives)}${marked}${
                  many ? `, ${used.length} of ${group.ids.length} used` : ''
                }`}
                title={
                  group.marks.length === 0 ? undefined : group.marks.map((mark) => mark.words).join('; ')
                }
                className={cn(
                  'h-8 gap-1.5 px-2 text-xs aria-disabled:pointer-events-none aria-disabled:opacity-50 aria-pressed:bg-primary/15',
                  switchable && 'rounded-r-none',
                )}
                onClick={() => {
                  if (target !== undefined) onAct(target)
                }}
              >
                <span aria-hidden="true">{group.name}</span>
                {many ? (
                  <span aria-hidden="true" className="text-muted-foreground tabular-nums">
                    {used.length > 0 ? `${used.length}/${group.ids.length}` : `×${group.ids.length}`}
                  </span>
                ) : null}
                <ManaMade gives={group.gives} sizeClass={PIP_SIZE} words />
                {group.marks.map((mark) => (
                  <span
                    key={mark.key}
                    aria-hidden="true"
                    data-mark={mark.key}
                    className="rounded-sm bg-muted px-1 text-xs text-muted-foreground"
                  >
                    {mark.short}
                  </span>
                ))}
              </Button>
              {switchable ? (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  data-slot="pay-switch"
                  aria-label={`Switch the colour of ${group.name}`}
                  title={`Switch the colour of ${group.name}`}
                  className="-ml-px h-8 rounded-l-none px-1.5"
                  onClick={() => onSwitch?.(swapped)}
                >
                  <ArrowLeftRight aria-hidden="true" className="size-3.5" />
                </Button>
              ) : null}
            </span>
            {here === undefined ? null : (
              <ManaChoice
                key={`${here.card}-${String(here.swap)}`}
                card={here.card}
                options={here.options}
                onPick={(option) => onPick?.(option)}
                onCancel={() => onClose?.()}
                compact
                verb={here.swap ? 'Switch to' : 'Add'}
              />
            )}
          </span>
        )
      })}
    </Row>
  )
}
