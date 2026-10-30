// Goal: the marks of a host say what each attachment gives in the tuck order: every rule, the sums of power and toughness, the set value, keywords, merged duplicates, the "+N" cap and the compact count, and the spoken words list each attachment with its gives.
import { describe, expect, it } from 'vitest'
import type { GivenRule, GivesView, ObjectView } from '@/protocol/generated'
import { card } from '@/test-utils/cards'
import {
  attachName,
  attachedWords,
  attachesOf,
  capMarks,
  compactTone,
  givesWords,
  itemsOf,
  listWords,
  marksFrom,
  marksOf,
  roomFor,
  shortKeyword,
} from './attachMarks'
import type { MarkTone } from './attachMarks'

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
const RULE_MARKS: Record<GivenRule, [text: string, words: string, tone: MarkTone]> = {
  CantAttack: ["Can't attack", "can't attack", 'restrict'],
  CantAttackYou: ["Can't attack you", "can't attack its controller", 'restrict'],
  CantBlock: ["Can't block", "can't block", 'restrict'],
  CanBlockOnly: ['Blocks few', 'can block only some attackers', 'restrict'],
  CantBeBlocked: ['Unblockable', "can't be blocked", 'raise'],
  CantBeBlockedBy: ['Evasive', "can't be blocked by some creatures", 'raise'],
  BlockerLimit: ['Block limit', 'needs or allows only some blockers', 'plain'],
  BlocksAdditional: ['Extra blocks', 'can block additional creatures', 'raise'],
  DoesntUntap: ["Doesn't untap", "doesn't untap", 'restrict'],
  LoseAbilities: ['Loses abilities', 'loses all abilities', 'restrict'],
  LoseKeyword: ['Loses a keyword', 'loses a keyword', 'restrict'],
  GainAbility: ['New ability', 'gains an ability', 'raise'],
  SetController: ['Stolen', 'changes controller', 'restrict'],
  SwitchPowerToughness: ['Swaps P/T', 'power and toughness are switched', 'plain'],
  ChangeTypes: ['New types', 'changes its types', 'plain'],
  SetColors: ['New colour', 'changes its colour', 'plain'],
  Goaded: ['Goaded', 'is goaded', 'restrict'],
  UmbraArmor: ['Umbra', 'umbra armor, so it is not destroyed', 'raise'],
}

describe('itemsOf', () => {
  it.each(Object.entries(RULE_MARKS) as Array<[GivenRule, [string, string, MarkTone]]>)(
    'makes the rule %s a mark',
    (rule, [text, words, tone]) => {
      expect(itemsOf({ rules: [rule] })).toEqual([{ text, words, tone }])
    },
  )

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

  it('leaves items out that do not fit and says so', () => {
    const armor = piece(2, 'Ethereal Armor', 'Aura', { power: 3, toughness: 3, keywords: ['First strike'] })
    expect(marksOf(HOST, [armor], { chars: 18 })[0]).toMatchObject({ text: '+3/+3 First strike', cut: false })
    expect(marksOf(HOST, [armor], { chars: 12 })[0]).toMatchObject({ text: '+3/+3 …', cut: true })
    expect(marksOf(HOST, [armor], { chars: 2 })[0]).toMatchObject({ text: '+3/+3 …', cut: true })
  })

  it('does not count the host among its attachments', () => {
    expect(marksOf(HOST, [HOST])).toEqual([])
  })

  it('reads every attachment of a lone host without a mark when it has none', () => {
    expect(marksOf(HOST, [])).toEqual([])
    expect(marksFrom([])).toEqual([])
  })
})

describe('capMarks', () => {
  const marks = marksOf(
    HOST,
    [1, 2, 3, 4, 5].map((id) => piece(10 + id, `Aura ${id}`, 'Aura', { power: id })),
  )

  it.each([
    [6, 5, 0],
    [5, 5, 0],
    [4, 3, 2],
    [3, 2, 3],
    [2, 1, 4],
    [1, 0, 5],
  ])('fits %i lines with %i marks and "+%i"', (room, shown, more) => {
    const capped = capMarks(marks, room)
    expect(capped.shown).toHaveLength(shown)
    expect(capped.more).toBe(more)
  })

  it('counts the attachments left out, not the marks', () => {
    const merged = marksOf(HOST, [
      piece(2, 'A', 'Aura', { power: 1 }),
      piece(3, 'B', 'Aura', { power: 1 }),
      piece(4, 'C', 'Aura', { power: 1 }),
      piece(5, 'D', 'Aura', { power: 4 }),
    ])
    expect(merged).toHaveLength(2)
    expect(capMarks(merged, 1).more).toBe(4)
    expect(capMarks(merged, 2)).toMatchObject({ more: 0 })
    const third = marksOf(HOST, [piece(9, 'E', 'Aura', { power: 9 })])
    expect(capMarks([...merged, ...third], 2).more).toBe(2)
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
