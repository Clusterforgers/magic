// Goal: the target prompt's words are right for every count the engine can ask (one, up to some, a range), every stage of the choice (nothing, too few, enough, full), every source (a spell, a triggered ability, one the view does not hold) and every place an object can be (battlefield, stack, a graveyard, hand, exile); equal targets share a key and a list holds each target once.
import { describe, expect, it } from 'vitest'
import type { Target } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import {
  boundsWords,
  distinctTargets,
  objectWords,
  possessive,
  sameTarget,
  subjectWords,
  targetKey,
  targetsSentence,
} from './targetText'

const NAMES = ['Ana', 'Bo', 'Cy']
const seatName = (seat: number): string => NAMES[seat] ?? `Seat ${seat + 1}`
const object = (id: number, counter = 0): Target => ({ Object: { id, counter } })

describe('boundsWords', () => {
  it.each([
    [1, 1, '1 target'],
    [2, 2, '2 targets'],
    [0, 1, 'up to 1 target'],
    [0, 3, 'up to 3 targets'],
    [1, 3, '1 to 3 targets'],
    [2, 4, '2 to 4 targets'],
  ])('%i to %i reads "%s"', (min, max, words) => {
    expect(boundsWords(min, max)).toBe(words)
  })
})

describe('targetsSentence', () => {
  const asked = { subject: 'Lightning Bolt', wording: 'target creature or planeswalker' }
  it.each([
    [
      'nothing chosen',
      { ...asked, min: 1, max: 1, chosen: 0 },
      'Lightning Bolt: choose target creature or planeswalker.',
    ],
    [
      'no words from the engine',
      { subject: 'Doom Blade', wording: '', min: 1, max: 1, chosen: 0 },
      'Doom Blade: choose 1 target.',
    ],
    [
      'no words and a range',
      { subject: 'Fire', wording: '', min: 0, max: 2, chosen: 0 },
      'Fire: choose up to 2 targets.',
    ],
    [
      'the one target chosen',
      { ...asked, min: 1, max: 1, chosen: 1 },
      'Lightning Bolt: 1 target chosen. Confirm the target, or choose another.',
    ],
    [
      'too few chosen',
      { subject: 'Fire', wording: '', min: 3, max: 4, chosen: 1 },
      'Fire: 1 of 3 targets chosen. Choose 2 more.',
    ],
    [
      'enough chosen with room for more',
      { subject: 'Fire', wording: '', min: 1, max: 3, chosen: 1 },
      'Fire: 1 target chosen. Confirm the targets, or choose up to 2 more.',
    ],
    [
      'the most chosen',
      { subject: 'Fire', wording: '', min: 0, max: 2, chosen: 2 },
      'Fire: 2 targets chosen. Confirm the targets, or take one back.',
    ],
    [
      'up to some and none chosen yet',
      { subject: 'Fire', wording: 'up to two target creatures', min: 0, max: 2, chosen: 0 },
      'Fire: choose up to two target creatures.',
    ],
  ])('says, with %s, "%s"', (_stage, input, sentence) => {
    expect(targetsSentence(input)).toBe(sentence)
  })
})

describe('possessive', () => {
  it.each([
    ['Mulldrifter', "Mulldrifter's"],
    ['Grizzly Bears', "Grizzly Bears'"],
    ['Bo', "Bo's"],
    ['Elf 2', "Elf 2's"],
  ])('turns %s into %s', (name, words) => {
    expect(possessive(name)).toBe(words)
  })
})

describe('subjectWords', () => {
  it.each([
    ['Lightning Bolt', false, 'Lightning Bolt'],
    ['Mulldrifter', true, "Mulldrifter's ability"],
    ['Grizzly Bears', true, "Grizzly Bears' ability"],
    [undefined, false, 'Your spell'],
    [undefined, true, 'A triggered ability'],
  ])('names %s (triggered: %s) as "%s"', (name, triggered, subject) => {
    expect(subjectWords(name, triggered)).toBe(subject)
  })
})

describe('objectWords', () => {
  it.each([
    ['a creature on the battlefield', card({ controller: 1 }), 'Grizzly Bears 2/2 (Bo)'],
    ['a land', card({ name: 'Forest', power: null, toughness: null, controller: 0 }), 'Forest (Ana)'],
    ['a spell on the stack', card({ zone: 'Stack', controller: 2 }), 'Grizzly Bears 2/2 (on the stack, Cy)'],
    [
      'a card in a graveyard',
      card({ zone: { Graveyard: 1 }, owner: 1 }),
      "Grizzly Bears 2/2 (in Bo's graveyard)",
    ],
    ['a card in a hand', card({ zone: { Hand: 0 } }), "Grizzly Bears 2/2 (in Ana's hand)"],
    ['a card in a library', card({ zone: { Library: 2 } }), "Grizzly Bears 2/2 (in Cy's library)"],
    ['a card in exile', card({ zone: 'Exile' }), 'Grizzly Bears 2/2 (in exile)'],
    ['a card in a command zone', card({ zone: { Command: 1 } }), "Grizzly Bears 2/2 (in Bo's command zone)"],
    [
      'an ability on the stack',
      card({ zone: 'Stack', kind: 'Ability', power: null, toughness: null, controller: 1 }),
      "Grizzly Bears' ability (on the stack, Bo)",
    ],
  ])('names %s', (_what, view, words) => {
    expect(objectWords(view, view.name, seatName)).toBe(words)
  })

  it('uses the name it is given, which the prompt numbers when two objects share one', () => {
    expect(objectWords(card({ controller: 1 }), 'Grizzly Bears 2', seatName)).toBe('Grizzly Bears 2 2/2 (Bo)')
  })
})

describe('targets', () => {
  it('give equal targets one key and tell players from objects and a new object from the old one', () => {
    expect(targetKey({ Player: 1 })).toBe(targetKey({ Player: 1 }))
    expect(sameTarget({ Player: 1 }, { Player: 1 })).toBe(true)
    expect(sameTarget({ Player: 1 }, { Player: 2 })).toBe(false)
    expect(sameTarget({ Player: 1 }, object(1))).toBe(false)
    expect(sameTarget(object(5), object(5))).toBe(true)
    expect(sameTarget(object(5), object(5, 1))).toBe(false)
    expect(sameTarget(object(5), object(6))).toBe(false)
  })

  it('are listed once each, in the order they first appear', () => {
    const list = [{ Player: 2 }, object(7), { Player: 2 }, object(3), object(7), object(7, 1)]
    expect(distinctTargets(list)).toEqual([{ Player: 2 }, object(7), object(3), object(7, 1)])
  })
})
