/** Pure: the card menu's Control group, the commands of who controls a card and whether it may act. */
import type { ManualGroup, ManualItem, MenuInput } from './offers'

/**
 * Remove summoning sickness (a sick permanent the seat controls), Sacrifice and one "Give control to …" entry per other
 * seat (a permanent it controls), or one "Onto the battlefield for …" entry per other seat (a card it owns in its
 * graveyard or exile, not an instant or sorcery); null when the card offers none.
 */
export function controlGroup(input: MenuInput): ManualGroup | null {
  const { object, seat, off } = input
  const others = input.players.filter((player) => player !== seat)
  const zone = object.zone
  const kind = zone === 'Battlefield' ? 'battlefield' : zone === 'Exile' ? 'exile' : typeof zone === 'object' && 'Graveyard' in zone ? 'graveyard' : 'other'
  const nameOf = input.nameOf ?? ((player: number) => `Seat ${player + 1}`)
  const items: ManualItem[] = []
  const give = (label: (who: string) => string): void => {
    for (const to of others) {
      items.push({
        key: `give-${to}`,
        label: label(nameOf(to)),
        action: { GiveControl: { object: object.id, to } },
        unavailable: off,
      })
    }
  }
  if (kind === 'battlefield' && object.controller === seat) {
    if (object.permanent?.summoning_sick === true) {
      items.push({
        key: 'unsick',
        label: 'Remove summoning sickness',
        action: { RemoveSickness: { object: object.id } },
        unavailable: off,
      })
    }
    items.push({ key: 'sacrifice', label: 'Sacrifice', action: { Sacrifice: { object: object.id } }, unavailable: off })
    give((who) => `Give control to ${who}`)
  }
  const spell = object.types.includes('Instant') || object.types.includes('Sorcery')
  if ((kind === 'graveyard' || kind === 'exile') && object.owner === seat && !spell) {
    give((who) => `Onto the battlefield for ${who}`)
  }
  return items.length === 0 ? null : { id: 'control', label: 'Control', items }
}
