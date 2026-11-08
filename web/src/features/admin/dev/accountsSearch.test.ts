// Goal: the address of /dev/accounts is read leniently, so a wrong value never breaks the page.
import { describe, expect, it } from 'vitest'
import { parseDevAccountsSearch } from './accountsSearch'

describe('parseDevAccountsSearch', () => {
  it.each([
    [{}, {}],
    [{ many: '1' }, { many: true }],
    [
      { many: 1, names: 'long' },
      { many: true, names: 'long' },
    ],
    [{ many: 'yes', names: 'short' }, {}],
  ])('reads %j as %j', (raw, expected) => {
    expect(parseDevAccountsSearch(raw)).toEqual(expected)
  })
})
