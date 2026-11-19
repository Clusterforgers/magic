/**
 * The dev table's `?prompt=priority-blocked`: the viewer holds priority in another seat's upkeep and the offer says why
 * three of its hand spells cannot be cast, as the server does (`PriorityOptions.unplayable`): one more spell would break
 * the fight's One Voice modifier, one waits for sorcery speed, one costs more than the mana at hand. With `boss=` the
 * boss's modifier is One Voice, so its chip lights when the first is pressed.
 */
import type { PendingDecision } from '@/play/store'
import type { ObjectView, UnplayableCard } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** The modifier that blocks the first spell. */
export const ONE_VOICE = 'One Voice'

const ONE_VOICE_TEXT = "Each player can't cast more than one spell each turn."

/** The emblem the dev boss brings as this fight's modifier. */
const DEV_MODIFIER = 'Deep Pockets'

/** The fixture with the boss's modifier and its emblem as One Voice; a table without a boss is left as it is. */
export function withOneVoice(fixture: Fixture): Fixture {
  const { view } = fixture
  const boss = view.players.find((player) => player.boss !== undefined && player.boss !== null)
  if (boss?.boss === undefined || boss.boss === null) return fixture
  const command = new Set(boss.command.cards ?? [])
  const objects = view.objects.map((object) =>
    command.has(object.id) && object.name === DEV_MODIFIER
      ? { ...object, name: ONE_VOICE, rules_text: ONE_VOICE_TEXT }
      : object,
  )
  const players = view.players.map((player) =>
    player.seat === boss.seat && player.boss !== undefined && player.boss !== null
      ? { ...player, boss: { ...player.boss, modifier: { name: ONE_VOICE, text: ONE_VOICE_TEXT } } }
      : player,
  )
  return { ...fixture, view: { ...view, objects, players } }
}

const isLand = (object: ObjectView): boolean => object.types.includes('Land')
const isInstant = (object: ObjectView): boolean =>
  object.types.includes('Instant') || (object.keywords ?? []).includes('Flash')

/**
 * `base` with the viewer's hand spells left out of `castable` and explained: the first card that is not an instant
 * waits for sorcery speed, the next is held back by One Voice (its emblem named when the boss brings it), the next
 * costs more than the one mana at hand.
 */
export function withBlocked(
  base: (fixture: Fixture) => PendingDecision,
): (fixture: Fixture) => PendingDecision {
  return (fixture) => {
    const pending = base(fixture)
    const { kind } = pending.decision
    if (!('Priority' in kind)) return pending
    const { view } = fixture
    const hand = new Set(view.players[view.viewer]?.hand.cards ?? [])
    const spells = view.objects.filter((object) => hand.has(object.id) && !isLand(object))
    const slow = spells.find((object) => !isInstant(object))
    const rest = spells.filter((object) => object !== slow)
    const [held, dear] = rest
    const emblem = view.objects.find((object) => object.name === ONE_VOICE && object.kind === 'Emblem')
    const unplayable: UnplayableCard[] = [
      ...(slow === undefined
        ? []
        : [{ card: slow.id, reason: 'Timing' as const, why: 'SorcerySpeed' as const }]),
      ...(held === undefined
        ? []
        : [
            {
              card: held.id,
              reason: 'Restricted' as const,
              why: { ForbiddenBy: { source: emblem?.id, name: ONE_VOICE, limit: 1 } },
            },
          ]),
      ...(dear === undefined
        ? []
        : [
            {
              card: dear.id,
              reason: 'Mana' as const,
              why: { Mana: { cost: dear.mana_cost ?? [{ Generic: 3 }], at_hand: 1 } },
            },
          ]),
    ]
    const blocked = new Set(unplayable.map((entry) => entry.card))
    const castable = kind.Priority.castable.filter((id) => !blocked.has(id))
    return {
      ...pending,
      decision: { ...pending.decision, kind: { Priority: { ...kind.Priority, castable, unplayable } } },
    }
  }
}
