/**
 * The attachments scene of `/dev/table?equip=1`: five Auras and Equipment on one of the viewer's creatures (an umbra
 * among them), one Equipment on another, an Equipment on no host, an opponent's Aura on the viewer's creature, the
 * viewer's Aura on an opponent's creature and a Curse on an opponent.
 */
import type { Event, ObjectId, ObjectView, PlayerId, StateView } from '@/protocol/generated'
import { rowOf } from '../cardModel'
import type { Fixture } from './fixture'
import type { PlayScene } from './playScenes'

/** The attachments of the scene's main host, deepest first as the table tucks them. */
export const EQUIP_STACK = [
  'Ethereal Armor',
  'Rancor',
  'Spider Umbra',
  'Bonesplitter',
  'Lightning Greaves',
] as const

interface Piece {
  name: string
  subtype: 'Aura' | 'Equipment'
  controller: PlayerId
  host?: ObjectId
  player?: PlayerId
  keywords?: string[]
  curse?: true
}

const pieceView = (id: ObjectId, piece: Piece): ObjectView => ({
  id,
  owner: piece.controller,
  controller: piece.controller,
  zone: 'Battlefield',
  name: piece.name,
  mana_cost: null,
  supertypes: [],
  types: piece.subtype === 'Aura' ? ['Enchantment'] : ['Artifact'],
  subtypes: piece.curse === true ? ['Aura', 'Curse'] : [piece.subtype],
  colors: piece.subtype === 'Aura' ? ['White'] : [],
  power: null,
  toughness: null,
  abilities: [],
  permanent: { tapped: false, damage: 0, summoning_sick: false },
  ...(piece.keywords === undefined ? {} : { keywords: piece.keywords }),
  ...(piece.host === undefined ? {} : { attached_to: piece.host }),
  ...(piece.player === undefined ? {} : { attached_player: piece.player }),
})

const creaturesOf = (fixture: Fixture, seat: PlayerId): ObjectView[] =>
  fixture.view.objects.filter(
    (object) =>
      object.zone === 'Battlefield' &&
      object.controller === seat &&
      rowOf(object) === 'creatures' &&
      object.attached_to === undefined,
  )

/** Adds the attachments scene to the fixture's battlefield; a seat without creatures gets no attachments of its own. */
export function withEquip(fixture: Fixture): Fixture {
  const viewer = fixture.view.viewer
  const rival = fixture.view.players.map((player) => player.seat).find((seat) => seat !== viewer) ?? viewer
  const [main, second, third] = creaturesOf(fixture, viewer)
  const [theirs] = creaturesOf(fixture, rival)
  const pieces: Piece[] = []
  if (main !== undefined) {
    pieces.push(
      { name: 'Ethereal Armor', subtype: 'Aura', controller: viewer, host: main.id },
      { name: 'Rancor', subtype: 'Aura', controller: viewer, host: main.id },
      { name: 'Spider Umbra', subtype: 'Aura', controller: viewer, host: main.id, keywords: ['Umbra armor'] },
      { name: 'Bonesplitter', subtype: 'Equipment', controller: viewer, host: main.id },
      { name: 'Lightning Greaves', subtype: 'Equipment', controller: viewer, host: main.id },
    )
  }
  if (second !== undefined)
    pieces.push({ name: 'Short Sword', subtype: 'Equipment', controller: viewer, host: second.id })
  if (third !== undefined)
    pieces.push({ name: 'Pacifism', subtype: 'Aura', controller: rival, host: third.id })
  if (theirs !== undefined)
    pieces.push({ name: 'Eel Umbra', subtype: 'Aura', controller: viewer, host: theirs.id })
  pieces.push(
    { name: 'Sword of Fire and Ice', subtype: 'Equipment', controller: viewer },
    { name: 'Curse of Opulence', subtype: 'Aura', controller: viewer, player: rival, curse: true },
  )
  const top = Math.max(0, ...fixture.view.objects.map((object) => object.id))
  const added = pieces.map((piece, at) => pieceView(top + 1 + at, piece))
  const attachments = new Map<ObjectId, ObjectId[]>()
  for (const object of added) {
    if (object.attached_to === undefined) continue
    attachments.set(object.attached_to, [...(attachments.get(object.attached_to) ?? []), object.id])
  }
  const hosts = fixture.view.objects.map((object) => {
    const extra = attachments.get(object.id)
    return extra === undefined
      ? object
      : { ...object, attachments: [...(object.attachments ?? []), ...extra] }
  })
  const battlefield = [...(fixture.view.battlefield.cards ?? []), ...added.map((object) => object.id)]
  return {
    ...fixture,
    view: {
      ...fixture.view,
      objects: [...hosts, ...added],
      battlefield: { count: battlefield.length, cards: battlefield },
    },
  }
}

/** The viewer's creatures on the battlefield, in battlefield order. */
const creaturesAt = (view: StateView, seat: PlayerId): ObjectView[] => {
  const byId = new Map(view.objects.map((object) => [object.id, object]))
  return (view.battlefield.cards ?? []).flatMap((id) => {
    const object = byId.get(id)
    return object !== undefined && object.controller === seat && object.types.includes('Creature')
      ? [object]
      : []
  })
}

/** `list` without `id`, or with it in id order. */
const listed = (list: readonly ObjectId[] | undefined, id: ObjectId, present: boolean): ObjectId[] =>
  [...(list ?? []).filter((other) => other !== id), ...(present ? [id] : [])].sort((a, b) => a - b)

/**
 * The move of `?fx=equip`: the viewer's Equipment that hangs on a creature goes to the next creature of the viewer, so
 * the arrow runs from the old host to the new one, and pressing again carries it on around. Without an Equipment on a
 * creature it takes an unattached one, or brings a new one, onto the first creature.
 */
export function equipMove(scene: PlayScene): Event[] {
  const { next, viewer } = scene
  const hosts = creaturesAt(next, viewer)
  const gear = (object: ObjectView): boolean =>
    object.zone === 'Battlefield' && object.controller === viewer && object.subtypes.includes('Equipment')
  let sword = next.objects.find((object) => gear(object) && object.attached_to !== undefined)
  sword ??= next.objects.find(gear)
  if (sword === undefined) {
    const top = Math.max(0, ...next.objects.map((object) => object.id))
    sword = pieceView(top + 1, { name: 'Bonesplitter', subtype: 'Equipment', controller: viewer })
    scene.add(sword)
    next.battlefield = {
      count: next.battlefield.count + 1,
      cards: [...(next.battlefield.cards ?? []), sword.id],
    }
  }
  const was = sword.attached_to
  const at = hosts.findIndex((host) => host.id === was)
  const host = hosts[(at + 1) % hosts.length]
  if (host === undefined || host.id === was) return []
  const id = sword.id
  scene.edit(id, (object) => {
    object.attached_to = host.id
  })
  if (was !== undefined) {
    scene.edit(was, (object) => {
      const rest = listed(object.attachments, id, false)
      if (rest.length > 0) object.attachments = rest
      else delete object.attachments
    })
  }
  scene.edit(host.id, (object) => {
    object.attachments = listed(object.attachments, id, true)
  })
  return [{ Attached: { object: id, to: host.id } }]
}
