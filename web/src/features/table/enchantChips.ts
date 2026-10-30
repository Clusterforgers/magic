/** The chips of a player's pod and tile for the Auras and Curses attached to that player. Pure. */
import { useMemo } from 'react'
import { usePlay } from '@/features/game/gameContext'
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { givesWords, itemsOf } from './attachMarks'
import type { GiveItem } from './attachMarks'
import { shortName } from './commander'
import { useSeatNamer } from './names'

/** One chip: the Auras of one name that one seat controls on the enchanted player. */
export interface EnchantChip {
  /** The first of the Auras by id, which is also the card the chip previews. */
  key: ObjectId
  count: number
  /** "Curse of Opulence" or "Curse of Opulence ×2"; the figure is the count alone. */
  text: string
  /** "Curse of Opulence (Cy's) enchants Bo", and what it gives Bo after a colon, for a screen reader and the tooltip. */
  words: string
  /** What the Aura gives the player as the server reads it (`ObjectView.gives`): "Hexproof", "Pro creatures". */
  gives: GiveItem[]
}

/**
 * The chips for `seat`: one for each distinct Aura or Curse on the battlefield attached to that player, by name and
 * controller, in id order, with a count when several of one kind are attached.
 */
export function enchantChips(
  seat: PlayerId,
  objects: Readonly<Record<ObjectId, ObjectView>>,
  nameOf: (seat: PlayerId) => string,
): EnchantChip[] {
  const groups = new Map<string, ObjectView[]>()
  const attached = Object.values(objects)
    .filter((object) => object.zone === 'Battlefield' && object.attached_player === seat)
    .sort((a, b) => a.id - b.id)
  for (const object of attached) {
    const key = `${object.controller}:${object.name}`
    groups.set(key, [...(groups.get(key) ?? []), object])
  }
  return [...groups.values()].flatMap((group): EnchantChip[] => {
    const [first] = group
    if (first === undefined) return []
    const count = group.length
    const by = first.controller === seat ? '' : ` (${nameOf(first.controller)}'s)`
    const given = givesWords(first.gives)
    return [
      {
        key: first.id,
        count,
        text: count === 1 ? shortName(first.name) : `${shortName(first.name)} ×${count}`,
        words: `${count === 1 ? '' : `${count} × `}${first.name}${by} enchants ${nameOf(seat)}${given === null ? '' : `: ${given}`}`,
        gives: itemsOf(first.gives),
      },
    ]
  })
}

/** The chips of the Auras and Curses attached to a seat, worked out again only when the objects or names change. */
export function useEnchantChips(seat: PlayerId): EnchantChip[] {
  const objects = usePlay((state) => state.objectsById)
  const name = useSeatNamer()
  return useMemo(() => enchantChips(seat, objects, name), [seat, objects, name])
}
