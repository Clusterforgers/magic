// Goal: the save bar says in words whether the draft is saved, ready to save, being saved or held back, names only an unusable name as a blocker (legality issues are said but never stop a save), and a save on its way wins over everything.
import { describe, expect, it } from 'vitest'
import { saveStatus } from './saveStatus'
import type { SaveInput } from './saveStatus'

const input = (over: Partial<SaveInput> = {}): SaveInput => ({
  dirty: true,
  saving: false,
  name: 'Burn',
  issues: [],
  format: 'modern',
  ...over,
})

describe('saveStatus', () => {
  it.each([
    ['nothing changed', { dirty: false }, 'saved', 'All changes saved.', null],
    ['a change with a good name', {}, 'ready', 'Unsaved changes.', null],
    ['a change while a save is out', { saving: true }, 'saving', 'Saving…', 'A save is on its way.'],
    [
      'a change with a blank name',
      { name: '   ' },
      'blocked',
      'Cannot save yet. Enter a name for the deck.',
      'Enter a name for the deck.',
    ],
    [
      'a change with a name that is too long',
      { name: 'x'.repeat(101) },
      'blocked',
      'Cannot save yet. The name is at most 100 characters (this one has 101).',
      'The name is at most 100 characters (this one has 101).',
    ],
    ['a save out and a blank name', { saving: true, name: '' }, 'saving', 'Saving…', 'A save is on its way.'],
  ] as const)('reads %s as %s', (_what, over, kind, text, blocker) => {
    expect(saveStatus(input(over))).toEqual({ kind, text, blocker })
  })

  it.each([
    [
      [{ code: 'banned' }],
      'Unsaved changes. Not legal in Modern: 1 banned card; it can still be saved and played.',
    ],
    [
      [{ code: 'banned' }, { code: 'banned' }, { code: 'deck_size' }],
      'Unsaved changes. Not legal in Modern: 2 banned cards, deck size; it can still be saved and played.',
    ],
  ] as const)('says the legality issues %j and that they do not stop the save', (issues, text) => {
    const status = saveStatus(input({ issues }))
    expect(status).toEqual({ kind: 'ready', text, blocker: null })
  })

  it('says nothing of legality when there is nothing to save', () => {
    expect(saveStatus(input({ dirty: false, issues: [{ code: 'banned' }] })).text).toBe('All changes saved.')
  })
})
