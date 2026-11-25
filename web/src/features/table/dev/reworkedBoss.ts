/**
 * The dev table's boss (`?boss=0&rework=1|2`): the boss of `withBoss` with its parts: an innate at level III, this
 * fight's modifier, two copies of a relic, its Satchel of three reusable cards and one once card, its phases by round
 * (the begun one an emblem too), and a relic of an ally (else of the viewer). `1` is round 6, Phase 1 three rounds away; `2` is round
 * 10, Phase 1 in force and Phase 2 five rounds away, after two casts of Rat Out.
 */
import type { BossView, ObjectView, PlayerView } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** How far the reworked fight has come: before Phase 1, or with Phase 1 in force. */
export type DevRework = 1 | 2

const EMBLEMS = ['Brood III', 'Deep Pockets', 'Whetstone', 'Whetstone'] as const

/** The relic an ally brings, an emblem in its command zone. */
export const DEV_HERO_RELIC = 'Lantern of Embers'

/** `zone` with the card `id` added. */
function withCard(zone: PlayerView['command'], id: number): PlayerView['command'] {
  const cards = [...(zone.cards ?? []), id]
  return { ...zone, count: cards.length, cards }
}

const SATCHEL = [
  { name: 'Rat Out', reusable: true },
  { name: 'Feed the Swarm', reusable: true },
  { name: 'Village Rites', reusable: true },
  { name: 'Second Harvest', reusable: false },
] as const

const object = (id: number, seat: number, name: string, kind: ObjectView['kind']): ObjectView => ({
  id,
  owner: seat,
  controller: seat,
  zone: { Command: seat },
  kind,
  name,
  mana_cost: kind === 'Emblem' ? null : [{ Generic: 1 }, { Colored: 'Black' }],
  supertypes: [],
  types: kind === 'Emblem' ? [] : ['Instant'],
  subtypes: [],
  colors: kind === 'Emblem' ? [] : ['Black'],
  power: null,
  toughness: null,
  abilities: [],
  permanent: null,
})

/** The reworked boss's view at `stage`, its Satchel cards from id `first`. */
export function devReworkedView(stage: DevRework, first: number): BossView {
  const begun = stage === 2
  return {
    phases: ['The Nest Stirs', 'Plague Tide'],
    begun: begun ? 1 : 0,
    innate: {
      name: 'Brood III',
      text: 'At the beginning of your upkeep, create three 1/1 black Rat creature tokens.',
      level: 3,
    },
    modifier: {
      name: 'Deep Pockets',
      text: "At the beginning of each player's draw step, that player draws an additional card.",
    },
    round_phases: [
      {
        name: 'The Nest Stirs',
        text: 'When this phase begins, create a Rat for each creature the heroes control. Rats you control have menace.',
        round: 8,
        begun,
      },
      {
        name: 'Plague Tide',
        text: 'When this phase begins, double the number of Rats you control. Creature tokens you control get +2/+0.',
        round: 14,
        begun: false,
      },
    ],
    phase_in: begun ? 5 : 3,
    relics: [
      { name: 'Whetstone', text: 'Creatures you control get +1/+0.' },
      { name: 'Whetstone', text: 'Creatures you control get +1/+0.' },
    ],
    satchel: SATCHEL.map((card, at) => ({
      ...card,
      tax: begun && at === 0 ? 4 : 0,
      object: first + at,
    })),
  }
}

/** `moment` (a table with `withBoss`) with its boss reworked at `stage`. */
export function withReworkedBoss(moment: Fixture, stage: DevRework): Fixture {
  const boss = moment.view.players.find((player) => player.boss !== undefined)
  if (boss === undefined) return moment
  const first = Math.max(0, ...moment.view.objects.map((each) => each.id)) + 1
  const phases = stage === 2 ? ['The Nest Stirs'] : []
  const emblems = [...EMBLEMS, ...phases].map((name, at) => object(first + at, boss.seat, name, 'Emblem'))
  // An ally brings the relic where there is one, so the viewer's pod keeps the chips it has without one.
  const heroes = moment.view.players.filter((player) => player.boss === undefined)
  const hero = heroes.find((player) => player.seat !== moment.view.viewer) ?? heroes[0]
  const relic = object(first + emblems.length, hero?.seat ?? 0, DEV_HERO_RELIC, 'Emblem')
  const satchelFirst = relic.id + 1
  const satchel = SATCHEL.map((card, at) => object(satchelFirst + at, boss.seat, card.name, 'Card'))
  const command = [...(boss.command.cards ?? []), ...emblems.map((each) => each.id)]
  const reworked: PlayerView = {
    ...boss,
    life: 90,
    command: { count: command.length, cards: command },
    satchel: satchel.map((each) => each.id),
    boss: devReworkedView(stage, satchelFirst),
  }
  const players = moment.view.players.map((player) =>
    player.seat === boss.seat
      ? reworked
      : player.seat === hero?.seat
        ? { ...player, command: withCard(player.command, relic.id) }
        : player,
  )
  const objects = [...moment.view.objects, ...emblems, relic, ...satchel]
  const turn = { ...moment.view.turn, round: stage === 2 ? 10 : 6 }
  return { ...moment, view: { ...moment.view, players, objects, turn } }
}
