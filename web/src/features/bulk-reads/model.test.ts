// Goal: the word diff marks exactly the words one wording has and the other lacks and joins back to its input, and the bulk list groups cards by their rule in the server's order.
import { describe, expect, it } from 'vitest'
import type { BulkCard, BulkReads } from '@/protocol/generated'
import { wordDiff } from './diff'
import { groupByRule, wordingOnly } from './model'

const changed = (pieces: { text: string; changed: boolean }[]) =>
  pieces.filter((piece) => piece.changed).map((piece) => piece.text.trim())

describe('wordDiff', () => {
  it.each([
    [
      'Discard your hand.',
      'Discard X cards, where X is the number of cards in your hand.',
      [],
      ['X cards, where X is the number of cards in'],
    ],
    ['Tap this creature.', 'Tap ~.', ['this creature.'], ['~.']],
    ['Draw a card.', 'draw a card', [], []],
  ])('marks what differs in %j and %j', (left, right, onLeft, onRight) => {
    const diff = wordDiff(left, right)
    expect(changed(diff.left)).toEqual(onLeft)
    expect(changed(diff.right)).toEqual(onRight)
    expect(diff.left.map((piece) => piece.text).join('')).toBe(left)
    expect(diff.right.map((piece) => piece.text).join('')).toBe(right)
  })
})

const card = (name: string, rule: string): BulkCard => ({
  oracle_id: name,
  name,
  read_hash: 'h',
  rule,
  clauses: [{ printed: 'p', engine: 'e', rules: [rule] }],
})

describe('groupByRule', () => {
  it('keeps the rules in the order the server lists them and drops empty ones', () => {
    const reads: BulkReads = {
      rules: [
        { name: 'self_name', title: 'Name', summary: 's' },
        { name: 'pronoun', title: 'Pronouns', summary: 'p' },
        { name: 'and_or', title: 'and/or', summary: 'a' },
      ],
      cards: [card('B', 'and_or'), card('A', 'self_name'), card('C', 'and_or'), card('D', 'new_rule')],
    }
    const groups = groupByRule(reads)
    expect(groups.map((group) => [group.rule.name, group.cards.map((c) => c.name)])).toEqual([
      ['self_name', ['A']],
      ['and_or', ['B', 'C']],
      ['new_rule', ['D']],
    ])
    expect(groups[2]?.rule.title).toBe('new rule')
  })

  it('counts in words', () => {
    expect(wordingOnly(1, 'approved')).toBe('1 card approved')
    expect(wordingOnly(3, 'approved')).toBe('3 cards approved')
  })
})
