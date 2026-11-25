// Goal: a hand card whose cast or land play is not offered now but whose own ability is (cycling, typecycling, channel, ninjutsu, forecast, suspend) gets a cue naming that play with its mana, or the count when several, and no other card does: not one that can be cast or played too, a permanent, a card of the graveyard or an opponent's card; the cue rides on the card's note beside its reasons.
import { describe, expect, it } from 'vitest'
import type { AbilityRef, ActivatedView, ObjectView, PriorityOptions, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import { abilityCue, abilityOnlyCues, abilityPlays, withAbilityCues } from './activations'

const ME = 0
const THEM = 1
const ID = 10

const ref = (ability = 0): AbilityRef => ({ object: ID, ability })

const options = (over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  abilities: [ref()],
  can_act: true,
  can_pass: true,
  ...over,
})

const words = (cost: string, keyword?: string, ability = 0): ActivatedView => ({
  ability: ref(ability),
  cost,
  effect: 'Draw a card.',
  ...(keyword === undefined ? {} : { keyword }),
})

const placed = (zone: Zone, activated: ActivatedView[], seat = ME): ObjectView =>
  card({ id: ID, zone, owner: seat, controller: seat, activated })

const cuesOf = (object: ObjectView, offer: PriorityOptions): ReadonlyMap<number, string> => {
  const objects = { [ID]: object }
  return abilityOnlyCues(abilityPlays(offer, objects, ME), offer, objects, ME)
}

describe('the words of a cue', () => {
  it.each<[string, ActivatedView | undefined, string]>([
    ['cycling', words('{2}, Discard this card', 'Cycling {2}'), 'Cycling {2}'],
    ['typecycling', words('{2}, Discard this card', 'Plainscycling {2}'), 'Plainscycling {2}'],
    [
      'ninjutsu',
      words('{1}{U}, Return an unblocked attacker you control to hand', 'Ninjutsu {1}{U}'),
      'Ninjutsu {1}{U}',
    ],
    ['a keyword whose line has no mana', words('{1}{W}, Reveal this card', 'Forecast'), 'Forecast {1}{W}'],
    ['channel, an ability word', words('{1}{G}, Discard Boseiju, Who Endures'), 'Ability {1}{G}'],
    ['an ability with no mana', words('Discard this card'), 'Ability'],
    ['an ability the server did not word', undefined, 'Ability'],
  ])('%s', (_case, worded, want) => {
    expect(abilityCue(worded)).toBe(want)
  })
})

describe('which cards get a cue', () => {
  const cycling = [words('{2}, Discard this card', 'Cycling {2}')]
  it.each<[string, ObjectView, Partial<PriorityOptions>, string | undefined]>([
    ['a spell too dear to cast, with cycling', placed({ Hand: ME }, cycling), {}, 'Cycling {2}'],
    ['a spell that can be cast too', placed({ Hand: ME }, cycling), { castable: [ID] }, undefined],
    [
      'a cycling land that can be played too',
      placed({ Hand: ME }, cycling),
      { playable_lands: [ID] },
      undefined,
    ],
    [
      'a card with two abilities',
      placed({ Hand: ME }, [...cycling, words('{1}{G}, Discard this card', undefined, 1)]),
      { abilities: [ref(), ref(1)] },
      '2 abilities',
    ],
    ['a card to suspend', placed({ Hand: ME }, []), { abilities: [], suspendable: [ID] }, 'Suspend'],
    ['a commander in the command zone', placed({ Command: ME }, cycling), {}, 'Cycling {2}'],
    ['a permanent', placed('Battlefield', cycling), {}, undefined],
    ['a card of the graveyard', placed({ Graveyard: ME }, cycling), {}, undefined],
    ['an opponent card', placed({ Hand: THEM }, cycling, THEM), {}, undefined],
  ])('%s', (_case, object, over, want) => {
    expect(cuesOf(object, options(over)).get(ID)).toBe(want)
  })

  it('rides on the note the card has, or makes one', () => {
    const cues = new Map([
      [1, 'Cycling {2}'],
      [2, 'Channel {1}{G}'],
    ])
    const notes = withAbilityCues(
      new Map([[1, { words: 'Activate Cycling 2, not castable: needs mana' }]]),
      cues,
    )
    expect(notes.get(1)).toEqual({
      words: 'Activate Cycling 2, not castable: needs mana',
      ability: 'Cycling {2}',
    })
    expect(notes.get(2)).toEqual({ words: 'Channel {1}{G}', ability: 'Channel {1}{G}' })
  })
})
