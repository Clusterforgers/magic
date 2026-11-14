// Goal: the dev look switch reads `?look=` the same way for every address and remembered value, and wears or clears the look on `<html>`.
import { afterEach, describe, expect, it } from 'vitest'
import { chooseLook, installLook, lookOf } from './look'

describe('lookOf and chooseLook', () => {
  it.each([
    ['?look=a', null, 'a'],
    ['?look=b&cards=real', null, 'b'],
    ['?seats=4&look=c', 'a', 'c'],
    ['', 'b', 'b'],
    ['?look=z', 'c', 'c'],
    ['?look=off', 'a', undefined],
    ['', null, undefined],
    ['', 'nonsense', undefined],
  ] as const)('%s with %s remembered wears %s', (search, remembered, look) => {
    expect(chooseLook(search, remembered)).toBe(look)
  })

  it('names off apart from no answer', () => {
    expect(lookOf('?look=off')).toBe('off')
    expect(lookOf('?look=')).toBeUndefined()
  })
})

describe('installLook', () => {
  afterEach(() => {
    sessionStorage.clear()
    delete document.documentElement.dataset.look
    window.history.replaceState(null, '', '/')
  })

  it('wears the asked look and keeps it for the next page of the tab', () => {
    window.history.replaceState(null, '', '/dev/table?look=c')
    installLook()
    expect(document.documentElement.dataset.look).toBe('c')
    window.history.replaceState(null, '', '/dev/lobby')
    installLook()
    expect(document.documentElement.dataset.look).toBe('c')
  })

  it('clears the look with look=off', () => {
    window.history.replaceState(null, '', '/dev/table?look=a')
    installLook()
    window.history.replaceState(null, '', '/dev/table?look=off')
    installLook()
    expect(document.documentElement.dataset.look).toBeUndefined()
  })
})
