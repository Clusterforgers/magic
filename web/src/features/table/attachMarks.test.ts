// Goal: the marks of a host say what each attachment gives in the tuck order: every rule, the sums of power and toughness, the set value, keywords, merged duplicates, the "+N" cap and the compact count, and the spoken words list each attachment with its gives.
import { describe, expect, it } from 'vitest'
import type { GivenRule, GivesView, ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import {
  attachName,
  attachedWords,
  attachesOf,
  compactTone,
  foldMarks,
  givesWords,
  itemsOf,
  listWords,
  marksFrom,
  marksOf,
  roomFor,
  shortKeyword,
  sumGives,
} from './attachMarks'
import type { MarkTone } from './attachMarks'
import { markPx } from './markLayout'

const HOST = card({ id: 1, controller: 0 })

/** An attachment of the given kind (a subtype) with what it gives. */
const piece = (
  id: number,
  name: string,
  kind: 'Aura' | 'Equipment' | 'Fortification',
  gives?: GivesView,
  controller = 0,
): ObjectView =>
  card({
    id,
    name,
    controller,
    subtypes: [kind],
    types: [kind === 'Aura' ? 'Enchantment' : 'Artifact'],
    attached_to: 1,
    ...(gives === undefined ? {} : { gives }),
  })

const SEAT = (id: number): string => ['Ana', 'Bo', 'Cy'][id] ?? `Seat ${id + 1}`

// Every rule the engine names, with the mark it makes: a rule left out of this table fails the type check.
type RuleMark = [text: string, brief: string, words: string, tone: MarkTone]
const RULE_MARKS: Record<GivenRule, RuleMark> = {
  CantAttack: ["Can't attack", 'No attack', "can't attack", 'restrict'],
  CantAttackYou: ["Can't attack you", 'Spares you', "can't attack its controller", 'restrict'],
  CantBlock: ["Can't block", 'No block', "can't block", 'restrict'],
  CanBlockOnly: ['Blocks few', 'Blocks few', 'can block only some attackers', 'restrict'],
  CantBeBlocked: ['Unblockable', 'Evades', "can't be blocked", 'raise'],
  CantBeBlockedBy: ['Evasive', 'Evasive', "can't be blocked by some creatures", 'raise'],
  BlockerLimit: ['Block limit', 'Blockers', 'needs or allows only some blockers', 'plain'],
  BlocksAdditional: ['Extra blocks', 'More blocks', 'can block additional creatures', 'raise'],
  DoesntUntap: ["Doesn't untap", 'No untap', "doesn't untap", 'restrict'],
  CantBeTurnedFaceUp: ['Stays face down', 'Face down', "can't be turned face up", 'restrict'],
  LoseAbilities: ['Loses abilities', 'Blanked', 'loses all abilities', 'restrict'],
  LoseKeyword: ['Loses a keyword', 'No keyword', 'loses a keyword', 'restrict'],
  GainAbility: ['New ability', 'Ability', 'gains an ability', 'raise'],
  SetController: ['Stolen', 'Stolen', 'changes controller', 'restrict'],
  SwitchPowerToughness: ['Swaps P/T', 'P/T swap', 'power and toughness are switched', 'plain'],
  ChangeTypes: ['New types', 'Types', 'changes its types', 'plain'],
  SetColors: ['New colour', 'Colour', 'changes its colour', 'plain'],
  Goaded: ['Goaded', 'Goaded', 'is goaded', 'restrict'],
  UmbraArmor: ['Umbra', 'Umbra', 'umbra armor, so it is not destroyed', 'raise'],
}

describe('itemsOf', () => {
  it.each(Object.entries(RULE_MARKS) as Array<[GivenRule, RuleMark]>)(
    'makes the rule %s a mark',
    (rule, [text, brief, words, tone]) => {
      expect(itemsOf({ rules: [rule] })).toEqual([{ text, brief, words, tone }])
    },
  )

  it.each(Object.entries(RULE_MARKS) as Array<[GivenRule, RuleMark]>)(
    'writes the rule %s in a brief form no longer than its text, of whole short words',
    (rule, [text, brief]) => {
      expect(brief.length).toBeLessThanOrEqual(text.length)
      expect(markPx(brief)).toBeLessThanOrEqual(markPx(text))
      // A single word of the brief form fits the foot stack of a 100 px card (62 px) with its padding.
      for (const word of brief.split(' ')) expect(markPx(word), `${rule}: ${word}`).toBeLessThanOrEqual(62)
    },
  )

  it('writes the brief form of the long keywords and leaves the rest as they are', () => {
    expect(itemsOf({ keywords: ['Indestructible', 'Flying', 'Protection from multicolored'] })).toMatchObject(
      [
        { text: 'Indestructible', brief: 'Indestr.' },
        { text: 'Flying', brief: 'Flying' },
        { text: 'Pro multicolored', brief: 'Pro multi' },
      ],
    )
  })

  it.each<[string, GivesView | undefined, string[]]>([
    ['nothing', undefined, []],
    ['an empty gives', {}, []],
    ['a raise of both', { power: 2, toughness: 2 }, ['+2/+2']],
    ['a raise of power', { power: 2 }, ['+2/+0']],
    ['a raise of toughness', { toughness: 3 }, ['+0/+3']],
    ['a lowering', { power: -1, toughness: -1 }, ['-1/-1']],
    ['a mixed change', { power: 3, toughness: -3 }, ['+3/-3']],
    ['a power that cancels out', { power: 0, toughness: 0 }, []],
    ['a set value', { set: [1, 1] }, ['Set 1/1']],
    ['a set value and a raise', { set: [0, 4], power: 1 }, ['Set 0/4', '+1/+0']],
    ['a keyword', { keywords: ['Flying'] }, ['Flying']],
    ['a protection', { keywords: ['Protection from red'] }, ['Pro red']],
    ['two keywords', { keywords: ['Haste', 'Shroud'] }, ['Haste', 'Shroud']],
    [
      'everything, in the order of a mark',
      { power: 1, toughness: 1, set: [2, 2], keywords: ['Reach'], rules: ['UmbraArmor'] },
      ['Set 2/2', '+1/+1', 'Umbra', 'Reach'],
    ],
    ['both attack and block together', { rules: ['CantAttack', 'CantBlock'] }, ["Can't attack or block"]],
    ['block alone', { rules: ['CantBlock', 'Goaded'] }, ["Can't block", 'Goaded']],
  ])('reads %s', (_name, gives, texts) => {
    expect(itemsOf(gives).map((item) => item.text)).toEqual(texts)
  })

  it.each<[GivesView, MarkTone]>([
    [{ power: 1, toughness: 1 }, 'raise'],
    [{ power: 1 }, 'raise'],
    [{ power: -2, toughness: -2 }, 'restrict'],
    [{ power: 2, toughness: -1 }, 'plain'],
    [{ keywords: ['Flying'] }, 'plain'],
    [{ set: [1, 1] }, 'plain'],
  ])('tones %j as %s', (gives, tone) => {
    expect(itemsOf(gives)[0]?.tone).toBe(tone)
  })

  it('says each part in words', () => {
    const gives: GivesView = { set: [1, 1], power: 2, toughness: 2, keywords: ['Protection from red'] }
    expect(givesWords(gives)).toBe('power and toughness set to 1/1, +2/+2, protection from red')
    expect(givesWords({})).toBeNull()
    expect(givesWords(undefined)).toBeNull()
  })

  it.each([
    ['Protection from red', 'Pro red'],
    ['Protection from multicolored', 'Pro multicolored'],
    ['Flying', 'Flying'],
    ['First strike', 'First strike'],
  ])('shortens the keyword %s to %s', (name, short) => {
    expect(shortKeyword(name)).toBe(short)
  })
})

describe('marksOf', () => {
  it('has one mark per attachment in the tuck order: Auras, Equipment, Fortifications, each by id', () => {
    const marks = marksOf(HOST, [
      piece(9, 'Fortress', 'Fortification', { power: 0, toughness: 1 }),
      piece(8, 'Greaves', 'Equipment', { keywords: ['Haste'] }),
      piece(7, 'Rancor', 'Aura', { power: 2 }),
      piece(5, 'Eel Umbra', 'Aura', { rules: ['UmbraArmor'] }),
      piece(6, 'Bonesplitter', 'Equipment', { power: 2 }),
    ])
    // Rancor and Bonesplitter give the same: one mark, at the place of the first.
    expect(marks.map((mark) => mark.text)).toEqual(['Umbra', '+2/+0', 'Haste', '+0/+1'])
    expect(marks.map((mark) => mark.ids)).toEqual([[5], [7, 6], [8], [9]])
  })

  it('writes the items of one attachment in one mark and tones it by its worst part', () => {
    const [armor, pacifism, mixed] = marksOf(HOST, [
      piece(2, 'Ethereal Armor', 'Aura', { power: 3, toughness: 3, keywords: ['First strike'] }),
      piece(3, 'Pacifism', 'Aura', { rules: ['CantAttack', 'CantBlock'] }),
      piece(4, 'Hobble', 'Aura', { power: 2, rules: ['CantBlock'] }),
    ])
    expect(armor).toMatchObject({ text: '+3/+3 First strike', tone: 'raise' })
    expect(pacifism).toMatchObject({ text: "Can't attack or block", tone: 'restrict' })
    expect(mixed).toMatchObject({ text: "+2/+0 Can't block", tone: 'restrict' })
  })

  it('marks an attachment that gives nothing by its name alone', () => {
    const [mark] = marksOf(HOST, [piece(2, 'Sword of Fire and Ice', 'Equipment')])
    expect(mark).toMatchObject({ text: 'Sword of Fire and Ice', tone: 'plain', cut: false })
  })

  it('merges attachments that give the same and are controlled alike', () => {
    const marks = marksOf(HOST, [
      piece(2, 'Bonesplitter', 'Equipment', { power: 2 }),
      piece(3, 'Bonesplitter', 'Equipment', { power: 2 }),
      piece(4, 'Rancor', 'Aura', { power: 2 }),
      piece(5, 'Sword', 'Equipment'),
      piece(6, 'Sword', 'Equipment'),
    ])
    expect(marks.map((mark) => [mark.text, mark.ids])).toEqual([
      ['+2/+0', [4, 2, 3]],
      ['Sword', [5, 6]],
    ])
  })

  it('keeps apart what another seat controls, with its controllers', () => {
    const marks = marksOf(HOST, [
      piece(2, 'Pacifism', 'Aura', { rules: ['CantAttack'] }, 0),
      piece(3, 'Pacifism', 'Aura', { rules: ['CantAttack'] }, 1),
      piece(4, 'Pacifism', 'Aura', { rules: ['CantAttack'] }, 1),
    ])
    expect(marks.map((mark) => [mark.ids, mark.foreign])).toEqual([
      [[2], []],
      [
        [3, 4],
        [1, 1],
      ],
    ])
  })

  it.each<[string, number, number, string[], boolean]>([
    ['a wide mark in one line', 200, 2, ['+3/+3 First strike'], false],
    ['a mark of 75 px in two lines', 75, 2, ['+3/+3 First', 'strike'], false],
    ['a mark of 75 px in one line leaves the keyword out', 75, 1, ['+3/+3 …'], true],
    ['a mark narrower than its first item', 20, 1, ['+3/+3'], true],
  ])('writes %s', (_name, span, maxLines, lines, cut) => {
    const armor = piece(2, 'Ethereal Armor', 'Aura', { power: 3, toughness: 3, keywords: ['First strike'] })
    const [mark] = marksOf(HOST, [armor], { span, maxLines })
    expect(mark).toMatchObject({ lines, text: lines.join(' '), cut })
  })

  it('writes Pacifism as "Pacified" where the full words do not fit one line', () => {
    const pacifism = piece(2, 'Pacifism', 'Aura', { rules: ['CantAttack', 'CantBlock'] })
    expect(marksOf(HOST, [pacifism], { span: 93 })[0]?.lines).toEqual(['Pacified'])
    expect(marksOf(HOST, [pacifism], { span: 130 })[0]?.lines).toEqual(["Can't attack or block"])
  })

  it('does not count the host among its attachments', () => {
    expect(marksOf(HOST, [HOST])).toEqual([])
  })

  it('reads every attachment of a lone host without a mark when it has none', () => {
    expect(marksOf(HOST, [])).toEqual([])
    expect(marksFrom([])).toEqual([])
  })
})

describe('sumGives', () => {
  it.each<[string, Array<GivesView | undefined>, GivesView]>([
    ['powers and toughnesses added', [{ power: 2 }, { power: 3, toughness: 3 }], { power: 5, toughness: 3 }],
    ['a raise and a lowering that cancel', [{ power: 2 }, { power: -2 }], {}],
    [
      'the same keyword once, whatever its case',
      [{ keywords: ['Haste'] }, { keywords: ['haste', 'Shroud'] }],
      {
        keywords: ['Haste', 'Shroud'],
      },
    ],
    [
      'a rule once',
      [{ rules: ['CantBlock'] }, { rules: ['CantBlock', 'Goaded'] }],
      {
        rules: ['CantBlock', 'Goaded'],
      },
    ],
    ['the last set value', [{ set: [1, 1] }, { set: [0, 4] }, {}], { set: [0, 4] }],
    ['an attachment that gives nothing', [undefined, { power: 1 }, {}], { power: 1 }],
    ['nothing at all', [], {}],
  ])('adds up %s', (_name, all, sum) => {
    expect(sumGives(all)).toEqual(sum)
  })
})

describe('foldMarks', () => {
  const auras = [1, 2, 3, 4, 5].map((id) => piece(10 + id, `Aura ${id}`, 'Aura', { power: id }))
  const lines = (marks: readonly { lines: readonly string[] }[]): number =>
    marks.reduce((sum, mark) => sum + mark.lines.length, 0)

  it.each<[number, number, number, string | null]>([
    [6, 5, 0, null],
    [5, 5, 0, null],
    [4, 2, 3, '+12/+0'],
    [3, 1, 4, '+14/+0'],
    [2, 1, 4, '+14/+0'],
    [1, 0, 5, '+15/+0'],
  ])(
    'fits five attachments in %i lines as %i marks and the sum of %i others "%s"',
    (room, shown, count, text) => {
      const folded = foldMarks(attachesOf(HOST, auras), room)
      expect(folded.shown).toHaveLength(shown)
      expect(folded.rest?.count ?? 0).toBe(count)
      expect(folded.rest?.text ?? null).toBe(text)
      expect(lines(folded.shown) + (folded.rest?.lines.length ?? 0)).toBeLessThanOrEqual(room)
    },
  )

  it('sums what the marks past the room give, whole attachments of any kind', () => {
    const attaches = attachesOf(HOST, [
      piece(2, 'Ethereal Armor', 'Aura', { power: 3, toughness: 3, keywords: ['First strike'] }),
      piece(3, 'Rancor', 'Aura', { power: 2 }),
      piece(4, 'Bonesplitter', 'Equipment', { power: 1, toughness: 1 }),
      piece(5, 'Boots', 'Equipment', { power: 2, toughness: 2, keywords: ['Haste'] }),
      piece(6, 'Cloak', 'Equipment', { keywords: ['Shroud', 'haste'] }),
    ])
    const { shown, rest } = foldMarks(attaches, 3)
    expect(shown.map((mark) => mark.text)).toEqual(['+3/+3 First strike'])
    expect(rest).toMatchObject({ text: '+5/+3 Haste Shroud', count: 4, tone: 'raise' })
    expect(rest?.words).toBe('4 more attachments: +5/+3, haste and shroud')
    expect(rest?.ids).toEqual([3, 4, 5, 6])
  })

  it('sums attachments that give nothing as their count', () => {
    const attaches = attachesOf(
      HOST,
      [2, 3, 4, 5].map((id) => piece(id, `Sword ${id}`, 'Equipment')),
    )
    expect(foldMarks(attaches, 2).rest).toMatchObject({ text: '+3', count: 3, tone: 'plain' })
  })

  it('counts the attachments left out, not the marks', () => {
    const attaches = attachesOf(HOST, [
      piece(2, 'A', 'Aura', { power: 1 }),
      piece(3, 'B', 'Aura', { power: 1 }),
      piece(4, 'C', 'Aura', { power: 1 }),
      piece(5, 'D', 'Aura', { power: 4 }),
      piece(6, 'E', 'Aura', { power: 9 }),
    ])
    expect(marksFrom(attaches)).toHaveLength(3)
    expect(foldMarks(attaches, 3).rest).toBeNull()
    // The mark of the first three stands for three attachments; the other two are summed.
    const folded = foldMarks(attaches, 2)
    expect(folded.shown.map((mark) => mark.ids.length)).toEqual([3])
    expect(folded.rest).toMatchObject({ count: 2, text: '+13/+0' })
  })

  it('gives a mark a second line only while they all fit, else one line each', () => {
    const pair = attachesOf(HOST, [
      piece(2, 'Pacifism', 'Aura', { rules: ['CantAttack', 'CantBlock'] }),
      piece(3, 'Armor', 'Aura', { power: 3, toughness: 3, keywords: ['First strike'] }),
    ])
    const tight = { span: 75 }
    expect(foldMarks(pair, 4, tight).shown.map((mark) => mark.lines)).toEqual([
      ['Pacified'],
      ['+3/+3 First', 'strike'],
    ])
    expect(foldMarks(pair, 2, tight).shown.map((mark) => mark.lines)).toEqual([['Pacified'], ['+3/+3 …']])
  })

  it('keeps every line of every mark inside the span, never a cut word', () => {
    const attaches = attachesOf(HOST, [
      piece(2, 'Pacifism', 'Aura', { rules: ['CantAttack', 'CantBlock'] }),
      piece(3, 'Armor', 'Aura', { power: 3, toughness: 3, keywords: ['Indestructible', 'First strike'] }),
      piece(4, 'Sword of Fire and Ice', 'Equipment'),
    ])
    for (const span of [62, 74, 93, 150]) {
      for (const room of [2, 3, 4, 5]) {
        const { shown, rest } = foldMarks(attaches, room, { span })
        for (const line of [...shown, ...(rest === null ? [] : [rest])].flatMap((mark) => mark.lines)) {
          expect(markPx(line), `${span}px, ${room} lines: ${line}`).toBeLessThanOrEqual(span)
        }
      }
    }
  })
})

describe('the compact count', () => {
  it.each<[string, GivesView[], MarkTone]>([
    ['a raise', [{ power: 1 }], 'raise'],
    ['a restriction among raises', [{ power: 1 }, { rules: ['CantAttack'] }], 'restrict'],
    ['only keywords', [{ keywords: ['Flying'] }], 'plain'],
    ['nothing given', [{}], 'plain'],
  ])('shows the icon of %s', (_name, gives, tone) => {
    const marks = marksOf(
      HOST,
      gives.map((given, at) => piece(2 + at, `A${at}`, 'Aura', given)),
    )
    expect(compactTone(marks)).toBe(tone)
  })
})

describe('roomFor', () => {
  it.each([
    [140, 0, 3],
    [140, 26, 1],
    [140, 52, 0],
    [210, 0, 4],
    [224, 0, 5],
    [72, 0, 1],
    [100, 78, -2],
  ])('gives a face %i px high with %i px used by other marks %i lines', (height, usedPx, room) => {
    expect(roomFor(height, usedPx)).toBe(room)
  })
})

describe('the words', () => {
  it('lists each attachment with what it gives and who controls it', () => {
    const attaches = attachesOf(HOST, [
      piece(3, 'Pacifism', 'Aura', { rules: ['CantAttack', 'CantBlock'] }, 1),
      piece(2, 'Rancor', 'Aura', { power: 2, keywords: ['Trample'] }),
      piece(4, 'Sword', 'Equipment'),
    ])
    expect(attachedWords(attaches, SEAT)).toEqual([
      'Rancor attached giving +2/+0 and trample',
      "Bo's Pacifism attached giving can't attack or block",
      'Sword attached',
    ])
    expect(attachName(attaches[1] ?? { name: '', foreign: undefined }, SEAT)).toBe("Bo's Pacifism")
  })

  it.each([
    [[], ''],
    [['a'], 'a'],
    [['a', 'b'], 'a and b'],
    [['a', 'b', 'c'], 'a, b and c'],
  ])('joins %j as "%s"', (words, text) => {
    expect(listWords(words)).toBe(text)
  })
})
