/**
 * The dev table's `?prompt=priority-detail`: the viewer holds priority in its main phase and the offer says in detail why
 * six of its cards cannot be used, as the server does: an Aura in hand with nothing to enchant (`Why.Host`), an X spell
 * in hand whose every payable X leaves no target (`Why.XTarget`), an unearth card in the graveyard short of mana, an X
 * ability whose every payable X leaves no target, an ability paid only with mana of the chosen color (`Why.ChosenColor`)
 * and one short at its least X (`Why.LeastX`, the last four `PriorityOptions.inactive`). The log ends with an activation
 * taken back for its mana (`AnnouncementRewound`).
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
/** The X spell in hand with no target for any X the viewer can pay. */
export const DETAIL_X_SPELL = 'Entrancing Melody'
/** The permanent whose ability is paid only with red mana, which the viewer has too little of. */
export const DETAIL_CHOSEN = 'Throne of Eldraine'
/** The permanent whose ability takes at least one dream counter, which it lacks. */
export const DETAIL_LEAST_X = 'Rasputin, the Oneiromancer'

const AURA_TEXT = "Enchant Wall\nEnchanted Wall can attack as though it didn't have defender."
const UNEARTH_TEXT =
  'Unearth {B} ({B}: Return this card from your graveyard to the battlefield. It gains haste. Exile it at the beginning of the next end step or if it would leave the battlefield. Unearth only as a sorcery.)'
const GETH_TEXT =
  "Intimidate\n{X}{B}: Put target artifact or creature card with mana value X from an opponent's graveyard onto the battlefield tapped under your control. Then that player mills X cards."

const MELODY_TEXT = 'Gain control of target creature with mana value X.'
const THRONE_TEXT =
  'As Throne of Eldraine enters, choose a color.\n{T}: Add four mana of the chosen color. Spend this mana only to cast monocolored spells of that color.\n{3}, {T}: Draw two cards. Spend only mana of the chosen color to activate this ability.'
const RASPUTIN_TEXT =
  'At the beginning of your end step, put a dream counter on Rasputin.\n{T}, Remove X dream counters from Rasputin: Look at the top X cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order. X can\'t be 0.'

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
  const melody: ObjectView = {
    ...structuredClone(aura),
    id: next + 2,
    name: DETAIL_X_SPELL,
    types: ['Sorcery'],
    subtypes: [],
    colors: ['Blue'],
    power: null,
    toughness: null,
    rules_text: MELODY_TEXT,
    mana_cost: [{ Colored: 'Blue' }, { Colored: 'Blue' }],
  }
  const throne: ObjectView = {
    ...structuredClone(permanent),
    id: next + 3,
    name: DETAIL_CHOSEN,
    types: ['Artifact'],
    subtypes: [],
    supertypes: ['Legendary'],
    colors: [],
    power: null,
    toughness: null,
    rules_text: THRONE_TEXT,
    mana_cost: [{ Generic: 5 }],
  }
  const rasputin: ObjectView = {
    ...structuredClone(permanent),
    id: next + 4,
    name: DETAIL_LEAST_X,
    subtypes: ['Human', 'Wizard'],
    supertypes: ['Legendary'],
    rules_text: RASPUTIN_TEXT,
    mana_cost: [{ Generic: 1 }, { Colored: 'White' }, { Colored: 'Blue' }],
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
          hand: {
            ...player.hand,
            count: player.hand.count + 1,
            cards: [...(player.hand.cards ?? []), melody.id],
          },
        }
      : player,
  )
  const battlefield = {
    ...view.battlefield,
    cards: [...(view.battlefield.cards ?? []), geth.id, throne.id, rasputin.id],
  }
  return {
    ...fixture,
    view: { ...view, objects: [...objects, zombie, geth, melody, throne, rasputin], players, battlefield },
  }
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
    const melody = named(fixture, DETAIL_X_SPELL)
    const throne = named(fixture, DETAIL_CHOSEN)
    const rasputin = named(fixture, DETAIL_LEAST_X)
    if (
      !('Priority' in kind) ||
      aura === undefined ||
      zombie === undefined ||
      geth === undefined ||
      melody === undefined ||
      throne === undefined ||
      rasputin === undefined
    )
      return pending
    const unplayable: UnplayableCard[] = [
      ...(kind.Priority.unplayable ?? []),
      {
        card: aura.id,
        reason: 'Targets',
        why: { Host: { needs: 'a Wall on the battlefield', by: 'Enchant' } },
      },
      {
        card: melody.id,
        reason: 'Targets',
        why: {
          XTarget: { needs: 'a creature with mana value X on the battlefield', zone: 'Battlefield', least: 0, most: 2 },
        },
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
            needs: "an artifact or creature card with mana value X in an opponent's graveyard",
            zone: 'Graveyard',
            least: 0,
            most: 2,
          },
        },
      },
      {
        ability: { object: throne.id, ability: 2 },
        reason: 'Mana',
        why: {
          ChosenColor: {
            color: 'Red',
            cost: [{ Colored: 'Red' }, { Colored: 'Red' }, { Colored: 'Red' }],
            at_hand: 1,
          },
        },
      },
      {
        ability: { object: rasputin.id, ability: 1 },
        reason: 'Cost',
        why: { LeastX: { least: 1 } },
      },
    ]
    const castable = kind.Priority.castable.filter((id) => id !== aura.id && id !== melody.id)
    return {
      ...pending,
      decision: {
        ...pending.decision,
        kind: { Priority: { ...kind.Priority, castable, unplayable, inactive } },
      },
    }
  }
}
