/**
 * The moments of play for `?fx=` (docs/ui-design.md, Playing is legible): a cast, an activated ability, a resolution,
 * a counter, a land and a draw, each built as the batch the server would send from the table as it stands, so a
 * press plays again on the next card. Each works on any table: what it needs and does not find, it brings.
 */
import type {
  Event,
  HiddenEvent,
  ObjectId,
  ObjectView,
  PlayerId,
  Snapshot,
  StateView,
  Target,
  Zone,
  ZoneView,
} from '@/protocol/generated'

/** The table a move changes, and the means to change it (the `Scene` of `fxScenarios.ts`). */
export interface PlayScene {
  next: StateView
  viewer: PlayerId
  foe: PlayerId
  edit(id: ObjectId, change: (object: ObjectView) => void): void
  add(object: ObjectView): void
}

const without = (zone: ZoneView, id: ObjectId): ZoneView => ({
  count: Math.max(0, zone.count - 1),
  cards: zone.cards === null ? null : zone.cards.filter((card) => card !== id),
})

const withCard = (zone: ZoneView, id: ObjectId | null): ZoneView => ({
  count: zone.count + 1,
  cards: zone.cards === null || id === null ? zone.cards : [...zone.cards, id],
})

/** The zone list of the view that `zone` names, to read or replace. */
function listOf(view: StateView, zone: Zone): { get: () => ZoneView; set: (list: ZoneView) => void } | null {
  if (zone === 'Stack') return { get: () => view.stack, set: (list) => (view.stack = list) }
  if (zone === 'Battlefield') return { get: () => view.battlefield, set: (list) => (view.battlefield = list) }
  if (zone === 'Exile') return { get: () => view.exile, set: (list) => (view.exile = list) }
  if (zone === 'PhasedOut') return null
  const [key, seat] = Object.entries(zone)[0] as [string, PlayerId]
  const player = view.players[seat]
  if (player === undefined) return null
  const field = ({ Hand: 'hand', Library: 'library', Graveyard: 'graveyard', Command: 'command' } as const)[
    key as 'Hand' | 'Library' | 'Graveyard' | 'Command'
  ]
  return { get: () => player[field], set: (list) => (player[field] = list) }
}

/** Moves `id` from its zone to `to` in the zone lists and on the object. */
function relocate(scene: PlayScene, id: ObjectId, to: Zone): void {
  const object = scene.next.objects.find((candidate) => candidate.id === id)
  if (object === undefined) return
  const from = listOf(scene.next, object.zone)
  if (from !== null) from.set(without(from.get(), id))
  const into = listOf(scene.next, to)
  if (into !== null) into.set(withCard(into.get(), id))
  scene.edit(id, (target) => {
    target.zone = to
    target.permanent =
      to === 'Battlefield' ? (target.permanent ?? { tapped: false, damage: 0, summoning_sick: true }) : null
    if (to !== 'Stack') delete target.targets
  })
}

const objectsIn = (view: StateView, ids: readonly ObjectId[] | null): ObjectView[] =>
  (ids ?? []).flatMap((id) => view.objects.find((object) => object.id === id) ?? [])

const nextId = (view: StateView): ObjectId => Math.max(0, ...view.objects.map((object) => object.id)) + 1

const isPermanentCard = (object: ObjectView): boolean =>
  object.types.some((type) => type !== 'Instant' && type !== 'Sorcery')

/** A new card like `template`, owned by `seat`, in `zone`, listed there. */
function bring(
  scene: PlayScene,
  template: ObjectView,
  seat: PlayerId,
  zone: Zone,
  name?: string,
): ObjectView {
  const id = nextId(scene.next)
  const object: ObjectView = {
    ...structuredClone(template),
    id,
    owner: seat,
    controller: seat,
    zone,
    permanent: zone === 'Battlefield' ? { tapped: false, damage: 0, summoning_sick: true } : null,
    ...(name === undefined ? {} : { name }),
  }
  delete object.targets
  scene.add(object)
  const list = listOf(scene.next, zone)
  if (list !== null) list.set(withCard(list.get(), id))
  return object
}

/** Some card on the table to copy: the first of `prefer`, else any card. */
function templateOf(view: StateView, prefer: (object: ObjectView) => boolean): ObjectView | undefined {
  return view.objects.find((object) => object.kind !== 'Ability' && prefer(object)) ?? view.objects[0]
}

const creatureOf = (view: StateView, seat: PlayerId): ObjectView | undefined =>
  objectsIn(view, view.battlefield.cards).find(
    (object) => object.controller === seat && object.types.includes('Creature'),
  )

const aimAt = (object: ObjectView | undefined): Target[][] =>
  object === undefined ? [] : [[{ Object: { id: object.id, counter: 0 } }]]

/** The top of the stack, the item that resolves first. */
const topOf = (view: StateView): ObjectView | undefined => objectsIn(view, view.stack.cards).at(-1)

/** A spell of `seat` goes on the stack aimed at `target`: a card of the viewer's hand, else a new one. */
function castSpell(scene: PlayScene, seat: PlayerId, aimed: Target[][]): ObjectView {
  const { next, viewer } = scene
  const own = seat === viewer ? objectsIn(next, next.players[seat]?.hand.cards ?? null) : []
  const card =
    own.find((object) => !object.types.includes('Land')) ??
    bring(scene, templateOf(next, (object) => object.types.includes('Instant')) as ObjectView, seat, {
      Hand: seat,
    })
  relocate(scene, card.id, 'Stack')
  scene.edit(card.id, (object) => {
    if (aimed.length > 0) object.targets = aimed
  })
  return card
}

/** How `object` resolves: a permanent onto the battlefield, anything else to its owner's graveyard. */
function resolveInto(scene: PlayScene, object: ObjectView, events: Event[]): void {
  if (object.kind === 'Ability') {
    scene.next.stack = without(scene.next.stack, object.id)
    scene.next.objects = scene.next.objects.filter((candidate) => candidate.id !== object.id)
  } else if (isPermanentCard(object)) {
    relocate(scene, object.id, 'Battlefield')
    events.push({ SpellResolved: { object: object.id } })
  } else {
    relocate(scene, object.id, { Graveyard: object.owner })
  }
}

function snapshotOf(object: ObjectView): Snapshot {
  return {
    id: object.id,
    counter: 0,
    controller: object.controller,
    owner: object.owner,
    zone: object.zone,
    counters: [],
    attachments: [],
  } as unknown as Snapshot
}

/** The play moments, each the events of its batch; empty when the table had nothing to do it with. */
export const PLAY_MOVES = {
  // The viewer casts a spell from the hand at an opponent's creature (a new one when the hand holds none).
  cast(scene: PlayScene): Event[] {
    const targets = aimAt(creatureOf(scene.next, scene.foe))
    const card = castSpell(scene, scene.viewer, targets)
    return [{ SpellCast: { player: scene.viewer, object: card.id, choices: { targets } } }]
  },

  // An opponent casts a spell from their hand at a creature of the viewer's, and it stays on the stack.
  rival(scene: PlayScene): Event[] {
    const targets = aimAt(creatureOf(scene.next, scene.viewer))
    const card = castSpell(scene, scene.foe, targets)
    return [{ SpellCast: { player: scene.foe, object: card.id, choices: { targets } } }]
  },

  // A creature of the viewer activates an ability aimed at an opponent's creature.
  activate(scene: PlayScene): Event[] {
    const { next, viewer, foe } = scene
    const source = creatureOf(next, viewer)
    if (source === undefined) return []
    const ability: ObjectView = {
      ...structuredClone(source),
      id: nextId(next),
      zone: 'Stack',
      kind: 'Ability',
      source: source.id,
      types: [],
      colors: [],
      abilities: [],
      permanent: null,
      power: null,
      toughness: null,
      targets: aimAt(creatureOf(next, foe)),
    }
    scene.add(ability)
    next.stack = withCard(next.stack, ability.id)
    const ref = { id: source.id, counter: 0 }
    return [
      { AbilityActivated: { player: viewer, source: ref, ability: 0 } },
      {
        AbilityPutOnStack: {
          object: ability.id,
          source: ref,
          snapshot: snapshotOf(source),
          ability: 0,
          controller: viewer,
        },
      },
    ]
  },

  // The top of the stack resolves; on an empty stack the opponent casts a creature that resolves in the same batch.
  resolve(scene: PlayScene): Event[] {
    const events: Event[] = []
    let top = topOf(scene.next)
    if (top === undefined) {
      const template = templateOf(scene.next, (object) => object.types.includes('Creature'))
      if (template === undefined) return []
      top = bring(scene, template, scene.foe, { Hand: scene.foe })
      relocate(scene, top.id, 'Stack')
      events.push({ SpellCast: { player: scene.foe, object: top.id } })
    }
    events.push({ ResolutionBegan: { object: top.id } })
    // What it aims at, when that is a creature on the battlefield, dies of it; an instant that aims at nothing takes an
    // opponent's creature.
    const aimedAt = (top.targets ?? [])
      .flat()
      .flatMap((target) => ('Object' in target ? [target.Object.id] : []))[0]
    const victim =
      aimedAt === undefined && top.types.includes('Instant')
        ? creatureOf(scene.next, scene.foe)
        : scene.next.objects.find((object) => object.id === aimedAt)
    if (victim !== undefined && victim.zone === 'Battlefield' && victim.types.includes('Creature')) {
      relocate(scene, victim.id, { Graveyard: victim.owner })
      events.push({ CreatureDied: { object: victim.id, cause: 'LethalDamage' } })
    }
    resolveInto(scene, top, events)
    events.push({ ResolutionEnded: { object: top.id, outcome: 'Resolved' } })
    return events
  },

  // The viewer casts a counterspell at the top of the stack and it resolves at once, countering it.
  counter(scene: PlayScene): Event[] {
    const { next, viewer, foe } = scene
    const events: Event[] = []
    let victim = topOf(next)
    if (victim === undefined || victim.kind === 'Ability') {
      victim = castSpell(scene, foe, [])
      events.push({ SpellCast: { player: foe, object: victim.id } })
    }
    const template = templateOf(next, (object) => object.types.includes('Instant'))
    if (template === undefined) return []
    const counter = bring(scene, template, viewer, { Hand: viewer }, 'Counterspell')
    relocate(scene, counter.id, 'Stack')
    const targets: Target[][] = [[{ Object: { id: victim.id, counter: 0 } }]]
    scene.edit(counter.id, (object) => {
      object.targets = targets
    })
    events.push(
      { SpellCast: { player: viewer, object: counter.id, choices: { targets } } },
      { ResolutionBegan: { object: counter.id } },
      { ResolutionEnded: { object: victim.id, outcome: 'Countered' } },
    )
    relocate(scene, victim.id, { Graveyard: victim.owner })
    relocate(scene, counter.id, { Graveyard: viewer })
    events.push({ ResolutionEnded: { object: counter.id, outcome: 'Resolved' } })
    return events
  },

  // The viewer plays a land: one from the hand, else a new one like a land on the table.
  land(scene: PlayScene): Event[] {
    const { next, viewer } = scene
    const inHand = objectsIn(next, next.players[viewer]?.hand.cards ?? null).find((object) =>
      object.types.includes('Land'),
    )
    const template = templateOf(next, (object) => object.types.includes('Land'))
    const land =
      inHand ?? (template === undefined ? undefined : bring(scene, template, viewer, { Hand: viewer }))
    if (land === undefined) return []
    relocate(scene, land.id, 'Battlefield')
    scene.edit(land.id, (object) => {
      if (object.permanent !== null) object.permanent.summoning_sick = false
    })
    return [{ LandPlayed: { player: viewer, object: land.id } }]
  },

  // The viewer draws a card and the opponent draws one the viewer does not see.
  draw(scene: PlayScene): Event[] {
    const { next, viewer, foe } = scene
    const template = templateOf(next, (object) => object.zone !== 'Battlefield')
    const mine = next.players[viewer]
    const theirs = next.players[foe]
    if (template === undefined || mine === undefined || theirs === undefined) return []
    const card = bring(scene, template, viewer, { Hand: viewer })
    mine.library = { count: Math.max(0, mine.library.count - 1), cards: null }
    theirs.library = { count: Math.max(0, theirs.library.count - 1), cards: null }
    theirs.hand = withCard(theirs.hand, null)
    return [{ CardDrawn: { player: viewer, object: card.id } }]
  },
} satisfies Record<string, (scene: PlayScene) => Event[]>

/** The hidden event of the `draw` move: the opponent's draw, which the viewer does not see. */
export function hiddenDraw(scene: PlayScene): HiddenEvent {
  return { CardDrawn: { player: scene.foe } }
}
