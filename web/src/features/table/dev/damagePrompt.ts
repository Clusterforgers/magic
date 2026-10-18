/** The AssignDamage decisions of the dev table (`?prompt=damage`, `damage3`, `damage1`). */
import type { DamageOptions, ObjectView } from '@/protocol/generated'
import type { DevPrompt } from './prompts'

const isCreature = (object: ObjectView): boolean =>
  object.zone === 'Battlefield' && object.types.includes('Creature')

/**
 * The viewer's first creature divides its power (at least two for each blocker, so every row has something to show)
 * among `count` creatures of the next seat standing in for blockers. The engine asks only when there are two or more
 * blockers; one is here to see that corner too.
 */
export const assignDamage =
  (count: number): DevPrompt =>
  ({ view }) => {
    const viewer = view.viewer
    const attacker = view.objects.find((object) => isCreature(object) && object.controller === viewer)
    const opponent = (viewer + 1) % view.players.length
    const blockers = view.objects.filter((object) => isCreature(object) && object.controller === opponent)
    const options: DamageOptions = {
      attacker: attacker?.id ?? 0,
      power: Math.max(attacker?.power ?? 0, 2 * count),
      blockers: blockers.slice(0, count).map((object) => object.id),
    }
    return { id: 1, decision: { player: viewer, kind: { AssignDamage: options } } }
  }
