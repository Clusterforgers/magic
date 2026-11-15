/** Pure: the card menu's Control group, the commands of who controls a card and whether it may act. */
import { hiddenCard } from './hidden'
import type { ManualGroup, ManualItem, MenuInput } from './offers'

/** The seats still in the game but `except`, the acting seat first, then the others in seat order. */
export function seatsExcept(input: Pick<MenuInput, 'players' | 'seat'>, except: number): number[] {
  return input.players
    .filter((player) => player !== except)
    .sort((a, b) => Number(b === input.seat) - Number(a === input.seat) || a - b)
}

/**
 * Remove summoning sickness and Sacrifice (a permanent the seat controls), "Give control to …" or "Take control" for
 * every seat but the controller of any permanent, or "Onto the battlefield for …" for every seat but the owner of a card
 * in a graveyard or in exile that is no instant or sorcery and not face down out of sight; null when the card offers
 * none.
 */
export function controlGroup(input: MenuInput): ManualGroup | null {
  const { object, seat, off } = input
  const zone = object.zone
  const kind =
    zone === 'Battlefield'
      ? 'battlefield'
      : zone === 'Exile'
        ? 'exile'
        : typeof zone === 'object' && 'Graveyard' in zone
          ? 'graveyard'
          : 'other'
  const nameOf = input.nameOf ?? ((player: number) => `Seat ${player + 1}`)
  const items: ManualItem[] = []
  const give = (to: number, label: string): void => {
    items.push({
      key: `give-${to}`,
      label,
      action: { GiveControl: { object: object.id, to } },
      unavailable: off,
    })
  }
  if (kind === 'battlefield') {
    if (object.controller === seat) {
      if (object.permanent?.summoning_sick === true) {
        items.push({
          key: 'unsick',
          label: 'Remove summoning sickness',
          action: { RemoveSickness: { object: object.id } },
          unavailable: off,
        })
      }
      items.push({
        key: 'sacrifice',
        label: 'Sacrifice',
        action: { Sacrifice: { object: object.id } },
        unavailable: off,
      })
    }
    for (const to of seatsExcept(input, object.controller)) {
      give(to, to === seat ? 'Take control' : `Give control to ${nameOf(to)}`)
    }
  }
  const spell = object.types.includes('Instant') || object.types.includes('Sorcery')
  if ((kind === 'graveyard' || kind === 'exile') && !spell && !hiddenCard(input)) {
    for (const to of seatsExcept(input, object.owner)) {
      give(to, `Onto the battlefield for ${to === seat ? 'you' : nameOf(to)}`)
    }
  }
  return items.length === 0 ? null : { id: 'control', label: 'Control', items }
}
