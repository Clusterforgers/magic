import { Button } from '@/components/ui/button'
import type { ManaOption, ObjectId } from '@/protocol/generated'
import { givesOfOptions } from '../manaMade'
import type { Gives } from '../manaMade'
import { ManaMade, madeWords } from '../ManaMade'
import { Quiet, Row } from './PaymentSummary'

const PIP_SIZE = 'size-4 text-xs'

/** Permanents that give the same mana under the same name, which one chip stands for. */
export interface SourceGroup {
  name: string
  gives: Gives
  ids: readonly ObjectId[]
}

/** The mana sources grouped by name and by what they give, in the order the options list them. */
export function groupSources(
  sources: ReadonlyMap<ObjectId, readonly ManaOption[]>,
  nameOf: (id: ObjectId) => string,
): SourceGroup[] {
  const groups = new Map<string, { name: string; gives: Gives; ids: ObjectId[] }>()
  for (const [id, options] of sources) {
    const name = nameOf(id)
    const gives = givesOfOptions(options)
    const key = `${name}|${JSON.stringify(gives)}`
    const group = groups.get(key)
    if (group === undefined) groups.set(key, { name, gives, ids: [id] })
    else group.ids.push(id)
  }
  return [...groups.values()]
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
}

/**
 * The permanents the payment can tap, one chip each (equal ones share a chip with a count) with the symbols of what a
 * tap gives: `{C}{C}` for a Sol Ring, `{1}: {W}{B}` for a Signet, a five-colour disc for a source of any colour. A click
 * does what a click on the card does; a chip is pressed while the payment uses one of its permanents.
 */
export function ManaSources({ sources, nameOf, legal, taken, onAct }: ManaSourcesProps) {
  const groups = groupSources(sources, nameOf)
  return (
    <Row label="Mana sources">
      {groups.length === 0 ? <Quiet>none</Quiet> : null}
      {groups.map((group) => {
        const used = group.ids.filter((id) => taken.includes(id))
        const free = group.ids.find((id) => legal.includes(id) && !taken.includes(id))
        const target = free ?? used.at(-1)
        const many = group.ids.length > 1
        const pressed = used.length > 0
        return (
          <Button
            key={group.ids.join('-')}
            type="button"
            variant="outline"
            size="sm"
            data-slot="pay-source"
            // Not `disabled`: a button that goes dead under the keyboard would drop its focus.
            aria-disabled={target === undefined}
            aria-pressed={pressed}
            aria-label={`Tap ${many ? `a ${group.name}` : group.name} for ${madeWords(group.gives)}${
              many ? `, ${used.length} of ${group.ids.length} used` : ''
            }`}
            className="h-8 gap-1.5 px-2 text-xs aria-disabled:pointer-events-none aria-disabled:opacity-50 aria-pressed:bg-primary/15"
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
          </Button>
        )
      })}
    </Row>
  )
}
