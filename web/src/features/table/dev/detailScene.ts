/**
 * The dev table's `?prompt=priority-detail`: the viewer holds priority in its main phase and the offer says in detail why
 * three of its cards cannot be used, as the server does: an Aura in hand with nothing to enchant (`Why.Host`), an
 * unearth card in the graveyard short of mana and an X ability whose every payable X leaves no target (`Why.XTarget`,
 * both `PriorityOptions.inactive`). The log ends with an activation taken back for its mana (`AnnouncementRewound`).
 */
import type { PendingDecision } from '@/play/store'
import type { EventView, InactiveAbility, ObjectView, UnplayableCard } from '@/protocol/generated'
import type { Fixture } from './fixture'

/** The Aura in hand with no Wall to enchant. */
export const DETAIL_AURA = 'Animate Wall'
/** The graveyard card whose unearth costs more than the mana at hand. */
export const DETAIL_UNEARTH = 'Dregscape Zombie'
/** The permanent whose X ability has no target for any X the viewer can pay. */
export const DETAIL_X = 'Geth, Lord of the Vault'

const AURA_TEXT = "Enchant Wall\nEnchanted Wall can attack as though it didn't have defender."
const UNEARTH_TEXT =
  'Unearth {B} ({B}: Return this card from your graveyard to the battlefield. It gains haste. Exile it at the beginning of the next end step or if it would leave the battlefield. Unearth only as a sorcery.)'
const GETH_TEXT =
  "Intimidate\n{X}{B}: Put target artifact or creature card with mana value X from an opponent's graveyard onto the battlefield tapped under your control. Then that player mills X cards."

const ownedIn = (object: ObjectView, zone: 'Hand' | 'Graveyard', seat: number): boolean =>
  typeof object.zone === 'object' && zone in object.zone && Object.values(object.zone)[0] === seat

const named = (fixture: Fixture, name: string): ObjectView | undefined =>
  fixture.view.objects.find((object) => object.name === name)

/**
 * The fixture with the three cards: the viewer's first hand spell that is no creature becomes the Aura, a creature of
 * its hand is copied as the unearth card into its graveyard, and one of its creatures on the battlefield as Geth.
 */
export function withDetailCards(fixture: Fixture): Fixture {
  const { view } = fixture
  if (named(fixture, DETAIL_AURA) !== undefined) return fixture
  const seat = view.viewer
  const hand = view.objects.filter(
    (object) => ownedIn(object, 'Hand', seat) && !object.types.includes('Land'),
  )
  const creature = hand.find((object) => object.types.includes('Creature'))
  const aura = hand.find((object) => !object.types.includes('Creature'))
  const permanent = view.objects.find(
    (object) =>
      object.zone === 'Battlefield' && object.controller === seat && object.types.includes('Creature'),
  )
  if (creature === undefined || aura === undefined || permanent === undefined) return fixture
  const next = Math.max(...view.objects.map((object) => object.id)) + 1
  const zombie: ObjectView = {
    ...structuredClone(creature),
    id: next,
    name: DETAIL_UNEARTH,
    zone: { Graveyard: seat },
    subtypes: ['Zombie'],
    rules_text: UNEARTH_TEXT,
    mana_cost: [{ Generic: 1 }, { Colored: 'Black' }],
  }
  const geth: ObjectView = {
    ...structuredClone(permanent),
    id: next + 1,
    name: DETAIL_X,
    subtypes: ['Vampire'],
    supertypes: ['Legendary'],
    rules_text: GETH_TEXT,
    mana_cost: [{ Generic: 4 }, { Colored: 'Black' }, { Colored: 'Black' }],
  }
  const objects = view.objects.map((object) =>
    object.id === aura.id
      ? {
          ...object,
          name: DETAIL_AURA,
          types: ['Enchantment' as const],
          subtypes: ['Aura'],
          colors: ['White' as const],
          power: null,
          toughness: null,
          rules_text: AURA_TEXT,
          mana_cost: [{ Colored: 'White' as const }],
        }
      : object,
  )
  const players = view.players.map((player) =>
    player.seat === seat
      ? {
          ...player,
          graveyard: { ...player.graveyard, cards: [...(player.graveyard.cards ?? []), zombie.id] },
        }
      : player,
  )
  const battlefield = { ...view.battlefield, cards: [...(view.battlefield.cards ?? []), geth.id] }
  return { ...fixture, view: { ...view, objects: [...objects, zombie, geth], players, battlefield } }
}

/** The activation of Geth the viewer began and the engine took back for its mana, as the log tells it. */
export function detailRewound(fixture: Fixture): EventView | null {
  const geth = named(fixture, DETAIL_X)
  if (geth === undefined) return null
  return {
    Public: {
      AnnouncementRewound: {
        player: fixture.view.viewer,
        object: geth.id,
        reason: 'Mana',
        why: { Mana: { cost: [{ Generic: 3 }, { Colored: 'Black' }], at_hand: 2 } },
        activated: true,
      },
    },
  }
}

/** `base` with the three cards left out of the offer and explained. */
export function withDetail(
  base: (fixture: Fixture) => PendingDecision,
): (fixture: Fixture) => PendingDecision {
  return (fixture) => {
    const pending = base(fixture)
    const { kind } = pending.decision
    const aura = named(fixture, DETAIL_AURA)
    const zombie = named(fixture, DETAIL_UNEARTH)
    const geth = named(fixture, DETAIL_X)
    if (!('Priority' in kind) || aura === undefined || zombie === undefined || geth === undefined)
      return pending
    const unplayable: UnplayableCard[] = [
      ...(kind.Priority.unplayable ?? []),
      {
        card: aura.id,
        reason: 'Targets',
        why: { Host: { needs: 'a Wall on the battlefield', by: 'Enchant' } },
      },
    ]
    const inactive: InactiveAbility[] = [
      ...(kind.Priority.inactive ?? []),
      {
        ability: { object: zombie.id, ability: 0, source: { PrintedKeyword: 0 } },
        reason: 'Mana',
        why: { Mana: { cost: [{ Colored: 'Black' }], at_hand: 0, missing: ['Black'] } },
      },
      {
        ability: { object: geth.id, ability: 1 },
        reason: 'Targets',
        why: {
          XTarget: {
            needs: 'an artifact or creature card with mana value X an opponent owns in a graveyard',
            zone: 'Graveyard',
            least: 0,
            most: 2,
          },
        },
      },
    ]
    const castable = kind.Priority.castable.filter((id) => id !== aura.id)
    return {
      ...pending,
      decision: {
        ...pending.decision,
        kind: { Priority: { ...kind.Priority, castable, unplayable, inactive } },
      },
    }
  }
}
