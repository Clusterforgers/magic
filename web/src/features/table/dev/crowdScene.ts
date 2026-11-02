/**
 * The crowded table of `?crowd=1`: a stack of fourteen items (a board of triggers, a spell and a response) and an attack
 * of twelve creatures spread over three defenders with five blocks, to look at a crowded stack and a big combat.
 */
import type { AttackerView, ObjectId, ObjectView, PlayerId, StateView, Target } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** How many creatures the active seat attacks with. */
export const CROWD_ATTACKERS = 12

/** One stack item of the scene: an ability of a creature of `seat` (its `nth` creature), or a spell. */
interface Item {
  seat: 'active' | 'viewer' | 'third'
  nth: number
  text: string
  /** How many times it is on the stack in a row. */
  times: number
  /** Targets the opponents in turn, one each. */
  aims?: true
}

/** From the bottom up: the spell that started it, then the triggers it set off, then the viewer's response on top. */
const ITEMS: readonly Item[] = [
  { seat: 'active', nth: 0, text: 'Whenever another creature enters, you gain 1 life.', times: 6 },
  { seat: 'third', nth: 1, text: 'Whenever a creature dies, each opponent loses 1 life.', times: 4 },
  { seat: 'active', nth: 2, text: 'Deal 1 damage to any target.', times: 2, aims: true },
  { seat: 'viewer', nth: 0, text: 'Draw a card.', times: 1 },
]

const creaturesOf = (view: StateView, seat: PlayerId): ObjectView[] =>
  view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' && object.controller === seat && object.types.includes('Creature'),
  )

/** An ability of `source` on the stack, as the server shows one: named after its source, with no types or cost. */
function ability(id: ObjectId, source: ObjectView, text: string, targets: Target[]): ObjectView {
  return {
    id,
    owner: source.controller,
    controller: source.controller,
    zone: 'Stack',
    name: source.name,
    mana_cost: null,
    supertypes: [],
    types: [],
    subtypes: [],
    colors: [],
    power: null,
    toughness: null,
    abilities: [],
    permanent: null,
    kind: 'Ability',
    source: source.id,
    rules_text: text,
    ...(targets.length === 0 ? {} : { targets: [targets], targets_legal: [targets.map(() => true)] }),
  }
}

/** The fixture's stack with the triggers of `ITEMS` put on top of its spells, the viewer's response last. */
function crowdedStack(view: StateView): StateView {
  const seats = view.players.length
  const active = view.turn.active
  const seatOf = (item: Item): PlayerId =>
    item.seat === 'active' ? active : item.seat === 'viewer' ? view.viewer : (active + 2) % seats
  const others = (seat: PlayerId): PlayerId[] =>
    view.players.map((player) => player.seat).filter((one) => one !== seat)
  let next = Math.max(...view.objects.map((object) => object.id)) + 1
  const added: ObjectView[] = []
  for (const item of ITEMS) {
    const seat = seatOf(item)
    const source = creaturesOf(view, seat)[item.nth]
    if (source === undefined) continue
    for (let time = 0; time < item.times; time += 1) {
      const rival = others(seat)[time % Math.max(1, others(seat).length)]
      const targets: Target[] = item.aims === true && rival !== undefined ? [{ Player: rival }] : []
      added.push(ability(next, source, item.text, targets))
      next += 1
    }
  }
  // The zone lists the stack bottom first, so the newest items go last.
  const cards = [...(view.stack.cards ?? []), ...added.map((object) => object.id)]
  return {
    ...view,
    objects: [...view.objects, ...added],
    stack: { count: cards.length, cards },
  }
}

/** Twelve attackers of the active seat spread over the other seats in turn, the first two at each of them blocked. */
function crowdedCombat(view: StateView): StateView {
  const active = view.turn.active
  const defenders = view.players.map((player) => player.seat).filter((seat) => seat !== active)
  const attackers = creaturesOf(view, active).slice(0, CROWD_ATTACKERS)
  const used = new Set<ObjectId>()
  const rows: AttackerView[] = attackers.map((attacker, index) => {
    const defender = defenders[index % defenders.length] ?? active
    // The first two attackers at each defender are blocked, the very first by two creatures.
    const blocked = index < 2 * defenders.length
    const want = index === 0 ? 2 : blocked ? 1 : 0
    const blockers = creaturesOf(view, defender)
      .filter((creature) => !used.has(creature.id))
      .slice(0, want)
      .map((creature) => creature.id)
    for (const id of blockers) used.add(id)
    return { object: attacker.id, target: { Player: defender }, blocked, blockers }
  })
  const attacking = new Set(attackers.map((attacker) => attacker.id))
  const objects = view.objects.map((object): ObjectView =>
    attacking.has(object.id) && object.permanent !== null
      ? { ...object, permanent: { ...object.permanent, tapped: true, summoning_sick: false } }
      : object,
  )
  return {
    ...view,
    objects,
    combat: { attackers: rows, attackers_declared: true, blockers_declared: defenders },
  }
}

/** The fixture with the crowded stack and the big attack. */
export function withCrowd(fixture: Fixture): Fixture {
  return { ...fixture, view: crowdedStack(crowdedCombat(fixture.view)) }
}
