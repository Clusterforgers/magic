// Goal: every detail the engine gives (`Why`) reads as one plain sentence that names the card and what is missing, a target on the stack adds the Full Control hint with the card's own verb, a printed condition is quoted from the rules text, a spell limit (One Voice) names its rule and the spells cast, a commander short of mana names its tax, an Aura, bestow or mutate spell names the host it lacks, an X ability or spell names the target no payable X has, a cost short at its least X names that X and its mana, an ability paid only with the chosen color names the color and its mana, a reason without detail falls back to its plain line, and a refused play becomes the notice.
import { describe, expect, it } from 'vitest'
import type { ObjectView, Unplayable, Why } from '@/protocol/generated'
import { reasonLabel } from './handReason'
import { refusalNotice, whyHint, whyLine } from './whyWords'
import type { Subject } from './whyWords'

const cast = (name: string, rulesText: string | null = null): Subject => ({ name, rulesText, doing: 'cast' })
const activate = (name: string, rulesText: string | null = null): Subject => ({
  name,
  rulesText,
  doing: 'activate',
})

describe('whyLine', () => {
  it.each<[string, Subject, Unplayable, Why, string]>([
    [
      'a copy spell with no creature spell',
      cast('Double Major', 'Copy target creature spell you control, except the copy isn’t legendary.'),
      'Targets',
      { Target: { needs: 'a creature spell you control on the stack', zone: 'Stack' } },
      'Double Major needs a target: a creature spell you control on the stack. Cast a creature first and keep priority (Full Control) to copy it.',
    ],
    [
      'a counterspell with an empty stack',
      cast('Counterspell', 'Counter target spell.'),
      'Targets',
      { Target: { needs: 'a spell on the stack', zone: 'Stack' } },
      'Counterspell needs a target: a spell on the stack. Wait until one is cast, then counter it while it is on the stack.',
    ],
    [
      'a kill spell with no creature',
      cast('Doom Blade', 'Destroy target nonblack creature.'),
      'Targets',
      { Target: { needs: 'a nonblack creature on the battlefield', zone: 'Battlefield' } },
      'Doom Blade needs a target: a nonblack creature on the battlefield.',
    ],
    [
      'a graveyard target',
      cast('Raise Dead'),
      'Targets',
      { Target: { needs: 'a creature card in your graveyard', zone: 'Graveyard' } },
      'Raise Dead needs a target: a creature card in your graveyard.',
    ],
    [
      'too little mana',
      cast('Grizzly Bears'),
      'Mana',
      { Mana: { cost: [{ Generic: 1 }, { Colored: 'Green' }], at_hand: 1 } },
      'Needs {1}{G}: you have 1 mana available.',
    ],
    [
      'the wrong colour',
      cast('Grizzly Bears'),
      'Mana',
      { Mana: { cost: [{ Generic: 1 }, { Colored: 'Green' }], at_hand: 2, missing: ['Green'] } },
      'Needs {1}{G}: you have 2 mana available. Nothing you have makes {G}.',
    ],
    [
      'a commander with its tax',
      cast('Kenrith'),
      'Mana',
      { Mana: { cost: [{ Generic: 6 }, { Colored: 'White' }], at_hand: 5, tax: 2 } },
      'Needs {6}{W} with {2} commander tax: you have 5 mana available.',
    ],
    [
      'one spell a turn, from the fight’s modifier',
      cast('Opt'),
      'Restricted',
      { ForbiddenBy: { source: 40, name: 'One Voice', limit: 1 } },
      'One Voice: you have cast your spell this turn.',
    ],
    [
      'two spells a turn',
      cast('Opt'),
      'Restricted',
      { ForbiddenBy: { source: 41, name: 'Fires of Invention', limit: 2 } },
      'Fires of Invention: you have cast your 2 spells this turn.',
    ],
    [
      'a spell limit with no visible source',
      cast('Opt'),
      'Restricted',
      { ForbiddenBy: { limit: 1 } },
      'You have cast your spell this turn: no more are allowed.',
    ],
    [
      'sorcery speed',
      cast('Grizzly Bears'),
      'Timing',
      'SorcerySpeed',
      'Sorcery speed: only in your main phase with an empty stack.',
    ],
    [
      'a sorcery-speed ability',
      activate('Slow'),
      'Timing',
      'SorcerySpeed',
      'Activate only as a sorcery: in your main phase with an empty stack.',
    ],
    [
      'a printed condition',
      cast('Combat Trick', 'Cast this spell only during combat.\nDraw a card.'),
      'Restricted',
      'OwnCondition',
      'Cast this spell only during combat.',
    ],
    [
      'an ability condition',
      activate('Hermit', '{T}: Draw a card. Activate only if you have no cards in hand.'),
      'Restricted',
      'OwnCondition',
      'Activate only if you have no cards in hand.',
    ],
    [
      'a permanent that forbids',
      cast('Opt'),
      'Restricted',
      { ForbiddenBy: { source: 7, name: 'Grand Abolisher' } },
      'Grand Abolisher stops you casting it now.',
    ],
    [
      'an effect that forbids',
      activate('Tapper'),
      'Restricted',
      { ForbiddenBy: {} },
      'An effect stops you activating it now.',
    ],
    [
      'split second',
      cast('Opt'),
      'Restricted',
      { SplitSecond: { spell: 3, name: 'Sudden Shock' } },
      'Sudden Shock has split second: nothing can be cast or activated until it resolves.',
    ],
    [
      'summoning sickness',
      activate('Mother of Runes'),
      'Cost',
      'SummoningSick',
      'Mother of Runes came under your control this turn, so its {T} abilities wait until your next turn.',
    ],
    ['a tapped source', activate('Mother of Runes'), 'Cost', 'Tapped', 'Mother of Runes is tapped.'],
    [
      'a spent limit',
      activate('Once'),
      'Restricted',
      'LimitSpent',
      'Already used as often as it allows this turn.',
    ],
    [
      'an additional cost',
      cast(
        "Altar's Reap",
        'As an additional cost to cast this spell, sacrifice a creature.\nDraw two cards.',
      ),
      'Cost',
      'OtherCost',
      "You can't pay its cost. As an additional cost to cast this spell, sacrifice a creature.",
    ],
    [
      'no way to cast',
      cast('Ancestral Vision'),
      'Cost',
      'NoWay',
      "Ancestral Vision can't be cast from here.",
    ],
  ])('%s', (_, subject, reason, why, expected) => {
    expect(whyLine(subject, reason, why)).toBe(expected)
  })

  it.each<Unplayable>(['Timing', 'LandPlayed', 'Mana', 'Targets', 'Cost', 'Restricted'])(
    'falls back to the plain line of %s without detail',
    (reason) => {
      const line = whyLine(cast('Card'), reason, undefined)
      expect(line).toMatch(/\.$/)
      expect(line).not.toContain('Card')
    },
  )
})

describe('whyHint', () => {
  it('speaks only of targets on the stack', () => {
    expect(
      whyHint(cast('Doom Blade'), {
        Target: { needs: 'a creature on the battlefield', zone: 'Battlefield' },
      }),
    ).toBe(null)
    expect(whyHint(cast('Opt'), 'SorcerySpeed')).toBe(null)
    expect(
      whyHint(cast('Redirect'), { Target: { needs: 'a spell you control on the stack', zone: 'Stack' } }),
    ).toBe('Cast a spell first and keep priority (Full Control) to target it.')
  })
})

describe('refusalNotice', () => {
  const objects = {
    5: { name: 'Double Major', rules_text: 'Copy target creature spell you control.' } as ObjectView,
  }

  it('says the detail when the server gives one', () => {
    expect(
      refusalNotice(
        {
          object: 5,
          reason: 'Targets',
          why: { Target: { needs: 'a creature spell you control on the stack', zone: 'Stack' } },
        },
        objects,
      ),
    ).toBe(
      'Double Major needs a target: a creature spell you control on the stack. Cast a creature first and keep priority (Full Control) to copy it.',
    )
  })

  it('names the card and the plain reason otherwise', () => {
    expect(refusalNotice({ object: 5, reason: 'Mana' }, objects)).toBe(
      "Double Major can't be played now. Not enough mana.",
    )
    expect(refusalNotice({ object: 9, ability: { object: 9, ability: 0 }, reason: 'Cost' }, objects)).toBe(
      "That card can't be activated now. A cost cannot be paid.",
    )
  })
})

describe('the details of hosts and X', () => {
  it.each<[string, Subject, Why, string]>([
    [
      'an Aura with nothing to enchant',
      cast('Pacifism'),
      { Host: { needs: 'a creature on the battlefield', by: 'Enchant' } },
      'Pacifism needs a creature on the battlefield to enchant.',
    ],
    [
      'a bestow cast with no creature',
      cast('Boon Satyr'),
      { Host: { needs: 'a creature on the battlefield', by: 'Bestow' } },
      'To cast Boon Satyr for bestow, it needs a creature on the battlefield to enchant.',
    ],
    [
      'a mutate cast with nothing to merge with',
      cast('Dreamtail Heron'),
      { Host: { needs: 'a non-Human creature you own on the battlefield', by: 'Mutate' } },
      'To mutate, Dreamtail Heron needs a non-Human creature you own on the battlefield.',
    ],
    [
      'an X ability with a range of X',
      activate('Geth, Lord of the Vault'),
      {
        XTarget: {
          needs: "an artifact or creature card with mana value X in an opponent's graveyard",
          zone: 'Graveyard',
          least: 0,
          most: 2,
        },
      },
      "Geth, Lord of the Vault has no target for any X you can pay (X from 0 to 2): it needs an artifact or creature card with mana value X in an opponent's graveyard.",
    ],
    [
      'a -X ability with one X',
      activate('Tamiyo, Compleated Sage'),
      {
        XTarget: {
          needs: 'a creature card with mana value X in your graveyard',
          zone: 'Graveyard',
          least: 0,
          most: 0,
        },
      },
      'Tamiyo, Compleated Sage has no target for any X you can pay (X = 0): it needs a creature card with mana value X in your graveyard.',
    ],
  ])('says %s', (_name, subject, why, line) => {
    expect(whyLine(subject, 'Targets', why)).toBe(line)
  })

  it('names the missing host on the card foot', () => {
    expect(reasonLabel('Targets', { Host: { needs: 'a creature on the battlefield', by: 'Enchant' } })).toBe(
      'Nothing to enchant',
    )
    expect(reasonLabel('Targets', { Host: { needs: 'a creature', by: 'Mutate' } })).toBe(
      'Nothing to mutate onto',
    )
  })
})

describe('the last details', () => {
  it.each<[string, Subject, Unplayable, Why, string]>([
    [
      'a least X short of mana',
      activate('Life Keeper'),
      'Mana',
      { LeastX: { least: 1, cost: [{ Generic: 1 }, { Colored: 'Green' }], at_hand: 1 } },
      'X must be at least 1: that costs {1}{G}, and you have 1 mana available.',
    ],
    [
      'a least X short of counters',
      activate('Rasputin, the Oneiromancer'),
      'Cost',
      { LeastX: { least: 1 } },
      "X must be at least 1, and the rest of its cost can't be paid for that.",
    ],
    [
      'a source that chose no color',
      activate('Throne of Eldraine'),
      'Mana',
      { ChosenColor: { at_hand: 0 } },
      'Throne of Eldraine has no chosen color, and only mana of the chosen color can be spent to activate it.',
    ],
    [
      'too little mana of the chosen color',
      activate('Throne of Eldraine'),
      'Mana',
      {
        ChosenColor: {
          color: 'Red',
          cost: [{ Colored: 'Red' }, { Colored: 'Red' }, { Colored: 'Red' }],
          at_hand: 1,
        },
      },
      'Only red mana can be spent to activate it: needs {R}{R}{R}, and you have 1 red mana available.',
    ],
    [
      'a cost of another color than the chosen one',
      activate('Throne of Eldraine'),
      'Mana',
      { ChosenColor: { color: 'Green', at_hand: 2 } },
      'Only green mana can be spent to activate it, and its cost asks for another color.',
    ],
    [
      'an X spell with no target for any payable X',
      cast('Entrancing Melody', 'Gain control of target creature with mana value X.'),
      'Targets',
      { XTarget: { needs: 'a creature with mana value X on the battlefield', zone: 'Battlefield', least: 0, most: 2 } },
      'Entrancing Melody has no target for any X you can pay (X from 0 to 2): it needs a creature with mana value X on the battlefield.',
    ],
    [
      'a loyalty ability short of the mana added to it',
      activate('Jace Beleren'),
      'Mana',
      { Mana: { cost: [{ Generic: 2 }], at_hand: 1 } },
      'Needs {2}: you have 1 mana available.',
    ],
  ])('says %s', (_name, subject, reason, why, line) => {
    expect(whyLine(subject, reason, why)).toBe(line)
  })

  it('labels a least X short of counters as a cost', () => {
    expect(reasonLabel('Cost', { LeastX: { least: 1 } })).toBe("Can't pay")
  })
})
