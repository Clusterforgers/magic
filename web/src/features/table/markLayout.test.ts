// Goal: the words of a grant mark are laid into lines of whole words that fit the width: the full words in one line, else the short ones, else two lines, else a word wider than the mark in its abbreviation, else the first items and "…", and a word is never cut by the box.
import { describe, expect, it } from 'vitest'
import { abbreviate, glyphPx, layoutWords, markPx } from './markLayout'

const item = (text: string, brief = text) => ({ text, brief })
const ARMOR = [item('+3/+3'), item('First strike'), item("Can't attack or block", 'Pacified')]

/** The keywords a card can be given, by the names the engine writes. */
const KEYWORDS = [
  'Flying',
  'First strike',
  'Double strike',
  'Deathtouch',
  'Trample',
  'Lifelink',
  'Vigilance',
  'Haste',
  'Menace',
  'Defender',
  'Indestructible',
  'Hexproof',
  'Shroud',
  'Reach',
  'Flash',
  'Intimidate',
  'Fear',
  'Skulk',
  'Shadow',
  'Wither',
  'Infect',
  'Undying',
  'Persist',
  'Annihilator',
  'Changeling',
  'Prowess',
  'Cascade',
  'Convoke',
  'Landwalk',
  'Evolve',
  'Dethrone',
  'Mentor',
  'Extort',
  'Exalted',
  'Battle cry',
  'Split second',
  'Umbra armor',
]

/** The room a mark of `span` px has for a line of text. */
const room = (span: number) => (text: string) => markPx(text) + 1 <= span

describe('abbreviate', () => {
  it.each<[string, number, string | null]>([
    // The longest start that ends in a consonant and fits, with a period.
    ['Deathtouch', 62, 'Deatht.'],
    ['Deathtouch', 54, 'Deatht.'],
    ['Intimidate', 52, 'Intimid.'],
    ['Annihilator', 62, 'Annihil.'],
    ['Changeling', 62, 'Changel.'],
    ['Deathtouch', 40, 'Deat.'],
    // Nothing fits, or it is no word of letters.
    ['Deathtouch', 30, null],
    ['+3/+3', 20, null],
    ['{2}', 20, null],
    ['Pro', 20, null],
    ['', 62, null],
  ])('writes %s in %i px as %s', (word, span, short) => {
    expect(abbreviate(word, room(span))).toBe(short)
  })

  it('writes a start of the word, narrower than it, that fits, and ends in a period after a consonant', () => {
    for (const name of KEYWORDS.flatMap((keyword) => keyword.split(' '))) {
      for (const span of [40, 50, 62, 80]) {
        const short = abbreviate(name, room(span))
        if (short === null) continue
        expect(short.endsWith('.'), short).toBe(true)
        expect(name.startsWith(short.slice(0, -1)), `${short} of ${name}`).toBe(true)
        expect(markPx(short), `${short} against ${name}`).toBeLessThan(markPx(name))
        expect(room(span)(short), `${short} in ${span} px`).toBe(true)
        expect('aeiouAEIOU'.includes(short.charAt(short.length - 2)), short).toBe(false)
      }
    }
  })
})

describe('glyphPx', () => {
  // Widths measured in the browser (11 px semibold Geist): the estimate stays within 8 percent over them.
  it.each<[string, number]>([
    ["Can't attack or block", 109.9],
    ['No attack', 51.1],
    ['+3/+3 Flying', 66],
    ['Haste Shroud', 71.3],
    ['Indestructible', 74],
    ['Double strike', 70.1],
  ])('estimates "%s" at about %d px', (text, measured) => {
    expect(glyphPx(text)).toBeGreaterThanOrEqual(measured)
    expect(glyphPx(text)).toBeLessThanOrEqual(measured * 1.08)
  })

  it('adds the padding of a mark and scales with the text size', () => {
    expect(markPx('Flying')).toBeCloseTo(glyphPx('Flying') + 8)
    expect(markPx('Flying', 12)).toBeCloseTo((glyphPx('Flying') * 12) / 11 + 8)
  })
})

describe('layoutWords', () => {
  it.each<[string, number, number, string[], boolean]>([
    ['no limit: all in one line', Infinity, 2, ["+3/+3 First strike Can't attack or block"], false],
    ['wide: the full words in one line', 280, 2, ["+3/+3 First strike Can't attack or block"], false],
    ['a little narrower: the short words in one line', 160, 2, ['+3/+3 First strike Pacified'], false],
    ['narrower: the full words in two lines', 140, 2, ["+3/+3 First strike Can't", 'attack or block'], false],
    ['narrow: the short words in two lines', 100, 2, ['+3/+3 First', 'strike Pacified'], false],
    ['one line only: the first items and "…"', 100, 1, ['+3/+3 …'], true],
  ])('%s', (_name, span, maxLines, lines, cut) => {
    expect(layoutWords(ARMOR, { span, maxLines })).toEqual({ lines, cut })
  })

  it('leaves items out with "…" when the lines do not hold them, keeping the first', () => {
    expect(layoutWords(ARMOR, { span: 62, maxLines: 2 })).toEqual({ lines: ['+3/+3 …'], cut: true })
    expect(layoutWords(ARMOR, { span: 20, maxLines: 2 })).toEqual({ lines: ['+3/+3', '…'], cut: true })
    expect(layoutWords(ARMOR, { span: 20, maxLines: 1 })).toEqual({ lines: ['+3/+3'], cut: true })
  })

  it('drops the last words of a lone long name', () => {
    const name = [item('Sword of Fire and Ice')]
    expect(layoutWords(name, { span: 62, maxLines: 2 })).toEqual({ lines: ['Sword of', 'Fire …'], cut: true })
    expect(layoutWords(name, { span: 200 })).toEqual({ lines: ['Sword of Fire and Ice'], cut: false })
  })

  it('never cuts a word: every written word is a word of the items, or "…"', () => {
    const words = new Set(['+3/+3', 'First', 'strike', 'Pacified', "Can't", 'attack', 'or', 'block', '…'])
    for (const span of [40, 62, 74, 93, 120, 200]) {
      for (const maxLines of [1, 2, 3]) {
        const { lines } = layoutWords(ARMOR, { span, maxLines })
        for (const word of lines.flatMap((line) => line.split(' '))) expect(words.has(word), word).toBe(true)
      }
    }
  })

  it('keeps every line within the span unless a single word is wider than it', () => {
    for (const span of [62, 74, 93, 120]) {
      const { lines } = layoutWords(ARMOR, { span, maxLines: 2 })
      for (const line of lines) {
        expect(line.includes(' ') ? markPx(line) : 0, `${span}: ${line}`).toBeLessThanOrEqual(span)
      }
    }
  })

  it.each<[string, number, number, string[]]>([
    // The foot stack of a 100 px card is 62 px wide: "Deathtouch" is 73 px with its padding, so it takes a short form.
    ['a lone keyword wider than the stack is shortened, not cut by the box', 62, 2, ['Deatht.']],
    ['in one line as well', 62, 1, ['Deatht.']],
    ['a lone keyword keeps its whole word where the stack is wide enough', 80, 2, ['Deathtouch']],
    ['a lone keyword that is just wide enough keeps its whole word', 74.2, 1, ['Deathtouch']],
    ['a lone keyword that is just too wide is shortened', 74, 1, ['Deathtouc.']],
  ])('%s', (_name, span, maxLines, lines) => {
    expect(layoutWords([item('Deathtouch')], { span, maxLines })).toEqual({ lines, cut: false })
  })

  it('shortens only the word that is too wide, and keeps the other items', () => {
    expect(layoutWords([item('+3/+3'), item('Deathtouch')], { span: 62, maxLines: 2 })).toEqual({
      lines: ['+3/+3', 'Deatht.'],
      cut: false,
    })
    expect(layoutWords([item('Deathtouch'), item('Flying')], { span: 62, maxLines: 2 })).toEqual({
      lines: ['Deatht.', 'Flying'],
      cut: false,
    })
  })

  it('prefers the short form of the item to an abbreviation, and an abbreviation to leaving items out', () => {
    const items = [item('Indestructible', 'Indestr.')]
    expect(layoutWords(items, { span: 62, maxLines: 2 })).toEqual({ lines: ['Indestr.'], cut: false })
    const both = [item('Deathtouch'), item('Lifelink'), item('Flying')]
    expect(layoutWords(both, { span: 62, maxLines: 3 }).cut).toBe(false)
  })

  it('leaves a lone word that no short form fits to the old last resort, which says it is cut', () => {
    expect(layoutWords([item('Deathtouch')], { span: 20, maxLines: 1 })).toEqual({
      lines: ['Deathtouch'],
      cut: true,
    })
  })

  it('writes every keyword of the game as whole lines in the stack of any card from 100 px up', () => {
    for (const name of KEYWORDS) {
      const items = [item(name, name === 'Indestructible' ? 'Indestr.' : name)]
      for (const span of [62, 70, 80, 93, 124, 186]) {
        // Two words take their two lines; one line holds a single word.
        for (const maxLines of name.includes(' ') ? [2] : [1, 2]) {
          const { lines, cut } = layoutWords(items, { span, maxLines })
          expect(cut, `${name} in ${span} px`).toBe(false)
          for (const line of lines)
            expect(markPx(line) + 1, `${name}: ${line} in ${span} px`).toBeLessThanOrEqual(span)
        }
      }
    }
  })

  it('grows the text with the font size', () => {
    const one = layoutWords([item('+3/+3'), item('First strike')], { span: 120, fontPx: 11 })
    const big = layoutWords([item('+3/+3'), item('First strike')], { span: 120, fontPx: 12 })
    expect(one.lines).toHaveLength(1)
    expect(big.lines.length).toBeGreaterThanOrEqual(one.lines.length)
  })
})
