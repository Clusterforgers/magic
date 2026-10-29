/**
 * The attachments scene of `/dev/table?equip=1`: five Auras and Equipment on one of the viewer's creatures (an umbra
 * among them), one Equipment on another, an Equipment on no host, an opponent's Aura on the viewer's creature, the
 * viewer's Aura on an opponent's creature and a Curse on an opponent.
 */
import type { ObjectId, ObjectView, PlayerId } from '@/protocol/generated'
import { rowOf } from '../cardModel'
import type { Fixture } from './fixture'

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
