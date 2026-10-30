// Goal: the rule of the life counter holds for any run of taps: the pending change adds up within the limits of one `Life` action, the running total counts a change in flight once, and Undo drops what was not sent or reverts what the server took, only while life is where that change left it.
import { describe, expect, it } from 'vitest'
import { lifeWords, shownLife, signed, tapped, undoOf } from './lifeRun'
import type { SentLife } from './lifeRun'

describe('tapped', () => {
  it.each<[number, number, number]>([
    [0, -5, -5],
    [-5, 5, 0],
    [3, -1, 2],
    [0, 5, 5],
    [998, 5, 1000],
    [-998, -5, -1000],
  ])('%i with a tap of %i is %i', (pending, step, expected) => {
    expect(tapped(pending, step)).toBe(expected)
  })
})

describe('the words of a change', () => {
  it.each<[number, string, string]>([
    [5, '+5', 'Gain 5 life'],
    [1, '+1', 'Gain 1 life'],
    [-1, '-1', 'Lose 1 life'],
    [-5, '-5', 'Lose 5 life'],
  ])('%i reads "%s" and "%s"', (delta, short, words) => {
    expect(signed(delta)).toBe(short)
    expect(lifeWords(delta)).toBe(words)
  })
})

describe('the running total', () => {
  const sent: SentLife = { delta: -5, from: 40 }
  it.each<[string, number, SentLife | null, boolean, number, number]>([
    ['nothing in flight or pending', 40, null, false, 0, 40],
    ['pending taps count', 40, null, false, -3, 37],
    ['a change in flight counts until the server moves', 40, sent, true, 0, 35],
    ['a change in flight is not counted twice once life moved', 35, sent, true, 0, 35],
    ['a change in flight and pending taps add', 40, sent, true, 2, 37],
    ['a change that was answered is in the server life', 35, sent, false, 0, 35],
    ['a refused change is not counted', 40, sent, false, 0, 40],
  ])('%s', (_, life, change, sending, pending, expected) => {
    expect(shownLife(life, change, sending, pending)).toBe(expected)
  })
})

describe('Undo', () => {
  const sent: SentLife = { delta: -5, from: 40 }
  it.each<[string, number, SentLife | null, boolean, number, ReturnType<typeof undoOf>]>([
    ['nothing to undo', 0, null, false, 40, null],
    ['pending taps are dropped, whatever was sent before', -2, sent, true, 35, { kind: 'drop', change: -2 }],
    ['a change the server took is reverted', 0, sent, true, 35, { kind: 'revert', change: -5 }],
    ['a change still in flight cannot be reverted', 0, sent, false, 40, null],
    ['a change is not reverted once life moved elsewhere', 0, sent, true, 31, null],
    ['an undo is not undone', 0, { delta: 5, from: 35, undo: true }, true, 40, null],
  ])('%s', (_, pending, change, done, life, expected) => {
    expect(undoOf(pending, change, done, life)).toEqual(expected)
  })
})
