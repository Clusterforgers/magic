/** The words under the big card in the preview: where it is, who controls it, what it is doing in combat. */
import type { CombatView, ObjectId, ObjectView, Zone } from '@/protocol/generated'
import type { SeatNamer } from './names'

/** Where a card is, such as "On the battlefield" or "In Alice's graveyard". */
export function zoneText(zone: Zone, name: SeatNamer): string {
  if (typeof zone === 'string') {
    switch (zone) {
      case 'Battlefield':
        return 'On the battlefield'
      case 'Stack':
        return 'On the stack'
      case 'Exile':
        return 'In exile'
    }
  }
  if ('Hand' in zone) return `In ${name(zone.Hand)}'s hand`
  if ('Library' in zone) return `In ${name(zone.Library)}'s library`
  if ('Graveyard' in zone) return `In ${name(zone.Graveyard)}'s graveyard`
  return `In ${name(zone.Command)}'s command zone`
}

/** "Controlled by Alice", plus the owner when they differ. */
export function controlText(object: ObjectView, name: SeatNamer): string {
  const control = `Controlled by ${name(object.controller)}`
  return object.owner === object.controller ? control : `${control}, owned by ${name(object.owner)}`
}

/** What the card does in combat, such as "Attacking Bo" or "Blocking Grizzly Bears", or null. */
export function combatText(
  combat: CombatView,
  id: ObjectId,
  name: SeatNamer,
  cardName: (id: ObjectId) => string,
): string | null {
  for (const attacker of combat.attackers) {
    if (attacker.object === id) {
      const target =
        'Player' in attacker.target ? name(attacker.target.Player) : cardName(attacker.target.Planeswalker)
      return `Attacking ${target}${attacker.blocked ? ', blocked' : ''}`
    }
    if (attacker.blockers.includes(id)) return `Blocking ${cardName(attacker.object)}`
  }
  return null
}
