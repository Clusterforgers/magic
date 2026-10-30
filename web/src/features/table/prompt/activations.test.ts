// Goal: which cards the Priority offer lets the viewer activate an ability of, and how each play is worded: only the viewer's own permanents and cards of its own hand, graveyard, exile and command zone, never an opponent's card or a mana ability, none for a spectator, and the plays of a card in the order land or spell, ability, mana; and the notes that carry the abilities to the card's name and preview, laid over the hand's reasons.
import { describe, expect, it } from 'vitest'
import type { AbilityRef, ActivatedView, ObjectView, PriorityOptions, Zone } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import type { CardNote, CardNotes } from './marks'
import {
  abilityKey,
  abilityPlays,
  activateWords,
  activationNotes,
  mergeNotes,
  playsOf,
  spoken,
} from './activations'

const ME = 0
const THEM = 1

const options = (abilities: AbilityRef[], over: Partial<PriorityOptions> = {}): PriorityOptions => ({
  playable_lands: [],
  castable: [],
  mana_abilities: [],
  abilities,
  can_act: true,
  can_pass: true,
  ...over,
})

const ref = (object: number, ability = 0): AbilityRef => ({ object, ability })
const worded = (of: AbilityRef, over: Partial<ActivatedView> = {}): ActivatedView => ({
  ability: of,
  cost: '{2}, {T}',
  effect: 'Draw a card.',
  ...over,
})

/** `id` in `zone`, controlled and owned by `seat`, with `activated` as its words. */
function placed(
  id: number,
  zone: Zone,
  seat: number,
  activated?: ActivatedView[],
  over: Partial<ObjectView> = {},
): ObjectView {
  return card({
    id,
    zone,
    owner: seat,
    controller: seat,
    ...(activated === undefined ? {} : { activated }),
    ...over,
  })
}

const table = (...objects: ObjectView[]): Record<number, ObjectView> =>
  Object.fromEntries(objects.map((object) => [object.id, object]))

describe('which cards are offered an ability', () => {
  const cases: [string, ObjectView, number | null, boolean][] = [
    ['your permanent', placed(10, 'Battlefield', ME), ME, true],
    ['your card in your graveyard', placed(10, { Graveyard: ME }, ME), ME, true],
    ['your card in the exile', placed(10, 'Exile', ME), ME, true],
    ['your commander in the command zone', placed(10, { Command: ME }, ME), ME, true],
    ['a permanent you steal', placed(10, 'Battlefield', ME, undefined, { owner: THEM }), ME, true],
    [
      'a permanent of yours someone else controls',
      placed(10, 'Battlefield', THEM, undefined, { owner: ME }),
      ME,
      false,
    ],
    ['an opponent permanent', placed(10, 'Battlefield', THEM), ME, false],
    ['a card in an opponent graveyard', placed(10, { Graveyard: THEM }, THEM), ME, false],
    ['a card in an opponent command zone', placed(10, { Command: THEM }, THEM), ME, false],
    ['a card in your hand', placed(10, { Hand: ME }, ME), ME, true],
    ['a card in an opponent hand', placed(10, { Hand: THEM }, THEM), ME, false],
    ['a card in your library', placed(10, { Library: ME }, ME), ME, false],
    ['an object on the stack', placed(10, 'Stack', ME), ME, false],
    ['your permanent, seen by a spectator', placed(10, 'Battlefield', ME), null, false],
    ['a card the table does not know', placed(11, 'Battlefield', ME), ME, false],
  ]
  it.each(cases)('%s', (_name, object, seat, offered) => {
    const found = abilityPlays(options([ref(10)]), table(object), seat)
    expect([...found.keys()]).toEqual(offered ? [10] : [])
  })

  it('offers nothing when the offer lists no ability, or leaves the list out', () => {
    const objects = table(placed(10, 'Battlefield', ME))
    expect(abilityPlays(options([]), objects, ME).size).toBe(0)
    expect(abilityPlays({ ...options([]), abilities: undefined }, objects, ME).size).toBe(0)
  })

  it('leaves out an ability the card words as a mana ability, and keeps its other abilities', () => {
    const [mana, draw] = [ref(10, 0), ref(10, 1)]
    const objects = table(
      placed(10, 'Battlefield', ME, [
        worded(mana, { cost: '{T}', effect: 'Add {G}.', mana: true }),
        worded(draw),
      ]),
    )
    const found = abilityPlays(options([mana, draw]), objects, ME)
    expect(found.get(10)?.map((play) => play.action)).toEqual([{ Activate: draw }])
    expect(abilityPlays(options([mana]), objects, ME).size).toBe(0)
  })

  it('collects the abilities of one card in the order the offer lists them, and of several cards apart', () => {
    const objects = table(
      placed(10, 'Battlefield', ME),
      placed(11, { Graveyard: ME }, ME),
      placed(12, 'Battlefield', THEM),
    )
    const found = abilityPlays(options([ref(11), ref(10, 2), ref(12), ref(10, 1)]), objects, ME)
    expect([...found.keys()]).toEqual([11, 10])
    expect(found.get(10)?.map((play) => play.action)).toEqual([
      { Activate: ref(10, 2) },
      { Activate: ref(10, 1) },
    ])
  })

  it('tells abilities of one index apart by where they come from', () => {
    const printed = ref(10, 0)
    const granted: AbilityRef = { object: 10, ability: 0, source: { PrintedKeyword: 3 } }
    const objects = table(
      placed(10, 'Battlefield', ME, [
        worded(printed, { cost: '{T}', effect: 'Scry 1.' }),
        worded(granted, { cost: '', effect: 'Equip {1}', keyword: 'Equip {1}', unworded: true }),
      ]),
    )
    const plays = abilityPlays(options([granted, printed]), objects, ME).get(10)
    expect(plays?.map((play) => play.label)).toEqual(['Equip {1}', '{T}: Scry 1.'])
    expect(new Set(plays?.map((play) => play.key)).size).toBe(2)
  })
})

describe('how an ability is worded', () => {
  const lineOf = (activated: ActivatedView | undefined, of = ref(10, 1)) => {
    const objects = table(placed(10, 'Battlefield', ME, activated === undefined ? undefined : [activated]))
    return abilityPlays(options([of]), objects, ME).get(10)?.[0]
  }

  it.each<[string, ActivatedView | undefined, string, string | undefined, string]>([
    [
      'cost and effect',
      worded(ref(10, 1)),
      '{2}, {T}: Draw a card.',
      undefined,
      'Activate 2, tap: Draw a card.',
    ],
    [
      'an effect with no cost',
      worded(ref(10, 1), { cost: '' }),
      'Draw a card.',
      undefined,
      'Activate Draw a card.',
    ],
    [
      'a loyalty cost',
      worded(ref(10, 1), { cost: '+1', effect: 'Scry 2.' }),
      '+1: Scry 2.',
      undefined,
      'Activate +1: Scry 2.',
    ],
    [
      'a keyword, with the effect under it',
      worded(ref(10, 1), {
        cost: '{1}',
        effect: 'Attach to target creature you control.',
        keyword: 'Equip {1}',
      }),
      'Equip {1}',
      '{1}: Attach to target creature you control.',
      'Activate Equip 1',
    ],
    [
      'an ability the writer cannot word',
      worded(ref(10, 1), { cost: '', effect: 'Activated ability 2', unworded: true }),
      'Activated ability 2',
      undefined,
      'Activate Activated ability 2',
    ],
    ['no words at all', undefined, 'Activated ability 2', undefined, 'Activate Activated ability 2'],
  ])('%s', (_name, activated, label, detail, words) => {
    const play = lineOf(activated)
    expect(play?.label).toBe(label)
    expect(play?.detail).toBe(detail)
    expect(play?.words).toBe(words)
  })

  it.each([
    ['{2}, {T}', '2, tap'],
    ['{W}{U}', 'white blue'],
    ['{3}{B}{B}', '3 black black'],
    ['{W/P}', 'white or life'],
    ['{R/G}: Add {C}.', 'red or green: Add colorless.'],
    ['Sacrifice Treasure', 'Sacrifice Treasure'],
  ])('reads %s aloud as %s', (text, aloud) => {
    expect(spoken(text)).toBe(aloud)
  })
})

describe('the plays of a card', () => {
  const objects = table(
    placed(10, 'Battlefield', ME, [worded(ref(10, 1)), worded(ref(10, 2), { cost: '', effect: 'Scry 1.' })]),
  )
  const found = abilityPlays(options([ref(10, 1), ref(10, 2)]), objects, ME)
  const tap = (ability: number, produces: 'Green' | 'Red' = 'Green') => ({
    source: { object: 10, ability },
    produces,
  })

  it('lists a spell first, then the abilities, then the mana', () => {
    const plays = playsOf(10, 'Gem', options([], { castable: [10] }), found, [tap(0, 'Red')])
    expect(plays.map((play) => play.action)).toEqual([
      { CastSpell: 10 },
      { Activate: ref(10, 1) },
      { Activate: ref(10, 2) },
      { ActivateMana: { object: 10, ability: 0 } },
    ])
    expect(plays.map((play) => play.words).at(0)).toBe('Cast Gem')
    expect(plays.map((play) => play.words).at(-1)).toBe('Add red mana')
  })

  it('plays a land and does not cast the card offered as both', () => {
    const plays = playsOf(10, 'Gem', options([], { playable_lands: [10], castable: [10] }), found, [])
    expect(plays.map((play) => play.action).at(0)).toEqual({ PlayLand: 10 })
    expect(plays).toHaveLength(3)
  })

  it('tells two mana abilities of one colour apart by their place on the card', () => {
    const plays = playsOf(10, 'Gem', options([]), found, [tap(0), tap(3)])
    expect(plays.slice(2).map((play) => play.words)).toEqual([
      'Add green (ability 1) mana',
      'Add green (ability 4) mana',
    ])
  })

  it('has the abilities alone for a graveyard card', () => {
    expect(playsOf(10, 'Gem', options([]), found, []).map((play) => play.key)).toHaveLength(2)
  })
})

describe('what a screen reader hears', () => {
  const objects = table(
    placed(10, 'Battlefield', ME, [
      worded(ref(10, 0), { cost: '{T}', effect: 'Scry 1.' }),
      worded(ref(10, 1)),
    ]),
    placed(11, 'Battlefield', ME, [worded(ref(11, 0), { cost: '{T}', effect: 'Scry 1.' })]),
  )
  const found = abilityPlays(options([ref(10, 0), ref(10, 1), ref(11, 0)]), objects, ME)

  it('names the one ability, or how many a card has', () => {
    expect(activateWords(found.get(11) ?? [])).toBe('Activate tap: Scry 1.')
    expect(activateWords(found.get(10) ?? [])).toBe('Activate one of 2 abilities')
  })

  it('gives every offered card a note and no other card', () => {
    const notes = activationNotes(found)
    expect([...notes.keys()].sort()).toEqual([10, 11])
    expect(notes.get(11)).toEqual({
      words: 'Activate tap: Scry 1.',
      usable: [abilityKey(ref(11, 0))],
    })
    expect(notes.get(10)?.usable).toEqual([abilityKey(ref(10, 0)), abilityKey(ref(10, 1))])
  })
})

describe('a hand card with an ability', () => {
  const cycling = worded(ref(10, 0), {
    cost: '',
    effect: 'Cycling {2}',
    keyword: 'Cycling {2}',
    unworded: true,
  })
  const objects = table(placed(10, { Hand: ME }, ME, [cycling]))
  const found = abilityPlays(options([ref(10, 0)]), objects, ME)

  it('lists its cast first and its cycling second', () => {
    const plays = playsOf(10, 'Cycler', options([ref(10, 0)], { castable: [10] }), found, [])
    expect(plays.map((play) => play.label)).toEqual(['Cast Cycler', 'Cycling {2}'])
    expect(plays.map((play) => play.action)).toEqual([{ CastSpell: 10 }, { Activate: ref(10, 0) }])
  })

  it('offers its cycling alone while it cannot be cast', () => {
    const plays = playsOf(10, 'Cycler', options([ref(10, 0)]), found, [])
    expect(plays.map((play) => play.words)).toEqual(['Activate Cycling 2'])
  })
})

describe('the notes laid over the hand reasons', () => {
  const reason: CardNote = { hint: 'Needs mana', line: 'Not enough mana', words: 'needs mana' }
  const doing: CardNote = { words: 'Activate Cycling 2', usable: ['k'] }
  const cases: [string, CardNotes, CardNotes, CardNotes][] = [
    [
      'no activation leaves the hand notes as they are',
      new Map([[1, reason]]),
      new Map(),
      new Map([[1, reason]]),
    ],
    [
      'an activation alone is the note of the card',
      new Map([[2, reason]]),
      new Map([[1, doing]]),
      new Map([
        [1, doing],
        [2, reason],
      ]),
    ],
    [
      'a card that cannot be cast keeps no hint, and its words and preview line say why',
      new Map([[1, reason]]),
      new Map([[1, doing]]),
      new Map([
        [
          1,
          {
            words: 'Activate Cycling 2, not castable: needs mana',
            line: 'Not castable: not enough mana',
            usable: ['k'],
          },
        ],
      ]),
    ],
    [
      'a reason with no preview line adds none',
      new Map([[1, { hint: 'Not now', words: 'not now' }]]),
      new Map([[1, doing]]),
      new Map([[1, { words: 'Activate Cycling 2, not castable: not now', usable: ['k'] }]]),
    ],
  ]
  it.each(cases)('%s', (_name, hand, activation, expected) => {
    expect(mergeNotes(hand, activation)).toEqual(expected)
  })
})
