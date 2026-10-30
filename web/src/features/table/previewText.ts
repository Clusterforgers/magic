/** The words under the big card in the preview: where it is, who controls it, what it is doing in combat. */
import type { CombatView, ObjectId, ObjectView, Zone } from '@/protocol/generated'
import { isUnattached } from '@/play/hosts'
import { attachName, givesWords } from './attachMarks'
import type { Attach } from './attachMarks'
import type { SeatNamer } from './names'
import { abilityKey } from './prompt/activations'

/** One attachment of a host in the preview: "Rancor: +2/+0, trample", "Bo's Pacifism: can't attack or block"; its name alone while it gives nothing. */
export function attachLine(attach: Attach, seat: SeatNamer): string {
  const name = attachName(attach, seat)
  return attach.items.length === 0 ? name : `${name}: ${attach.items.map((item) => item.words).join(', ')}`
}

/** An attachment's own line in the preview: what it hangs on and what it gives it, or null for a permanent that hangs on nothing. */
export function hostLine(object: ObjectView, host: string | undefined): string | null {
  if (host === undefined) return isUnattached(object) ? 'Not attached to anything' : null
  const words = givesWords(object.gives)
  const on = object.attached_player === undefined ? `Attached to ${host}` : `Enchants ${host}`
  return words === null ? on : `${on}, giving ${words}`
}

/** A creature's line in the preview when a soulbond pair holds it: "Paired with Grizzly Bears", or null while it is paired with no one. */
export function pairLine(partner: string | undefined): string | null {
  return partner === undefined ? null : `Paired with ${partner}`
}

/** Where a card is, such as "On the battlefield" or "In Alice's graveyard". */
export function zoneText(zone: Zone, name: SeatNamer): string {
  if (typeof zone === 'string') {
    switch (zone) {
      case 'Battlefield':
        return 'On the battlefield'
      case 'PhasedOut':
        return 'Phased out'
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

/** One activated ability of a card in the preview: its words, and whether the viewer can activate it now. */
export interface ActivatedLine {
  /** Tells the lines of one card apart. */
  key: string
  /** The ability as the card words it: `{2}, {T}: Draw a card.`, or `Equip {1} ({1}: Attach to target creature you control.)` for a keyword. */
  text: string
  /** True while the prompt offers it (`CardNote.usable`). */
  now: boolean
}

/**
 * The activated abilities of `object` in its card's words, in the order of the card: every one that is not a mana ability
 * (the preview's mana line tells those), with the ones named by `usable` (`abilityKey`) marked as available now. Empty
 * while the server sent no words, which it does for a card the viewer may not read.
 */
export function activatedLines(object: ObjectView, usable: readonly string[] = []): ActivatedLine[] {
  return (object.activated ?? []).flatMap((entry, index) => {
    if (entry.mana === true) return []
    const full = entry.cost === '' ? entry.effect : `${entry.cost}: ${entry.effect}`
    const text =
      entry.unworded === true
        ? entry.effect
        : entry.keyword === undefined
          ? full
          : `${entry.keyword} (${full})`
    const now = entry.ability !== undefined && usable.includes(abilityKey(entry.ability))
    return [{ key: `${index}-${text}`, text, now }]
  })
}
