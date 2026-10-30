// Goal: the words of a grant mark are laid into lines of whole words that fit the width: the full words in one line, else the short ones, else two lines, else the first items and "…", and a word is never cut.
import { describe, expect, it } from 'vitest'
import { glyphPx, layoutWords, markPx } from './markLayout'

const item = (text: string, brief = text) => ({ text, brief })
const ARMOR = [item('+3/+3'), item('First strike'), item("Can't attack or block", 'Pacified')]

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

  it('grows the text with the font size', () => {
    const one = layoutWords([item('+3/+3'), item('First strike')], { span: 120, fontPx: 11 })
    const big = layoutWords([item('+3/+3'), item('First strike')], { span: 120, fontPx: 12 })
    expect(one.lines).toHaveLength(1)
    expect(big.lines.length).toBeGreaterThanOrEqual(one.lines.length)
  })
})
