// Goal: a new game starts as Commander and the form names the rules that format plays by, the house rules that change them and the deck rules of the format.
import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { formatRuleWords } from '@/lib/houseRules'
import { openCreateForm } from '@/test-utils/createForm'

describe('the rules of a format, named', () => {
  it('lists what Commander plays by that the house rules can change', () => {
    expect(formatRuleWords('commander')).toEqual([
      'Starting life 40',
      '1 free mulligan',
      'Commander tax 2',
      'Commander damage 21',
      'Singleton decks',
      'Color identity enforced',
      'Decks of 100 cards',
    ])
  })
})

describe('the form', () => {
  it('says which rules a Commander game plays by, and drops the line for Normal', async () => {
    const { user } = await openCreateForm({}, 'default')
    expect(screen.getByRole('radio', { name: /^Commander/ })).toBeChecked()
    const line = document.querySelector('[data-slot="format-rules"]')
    expect(line).toHaveTextContent(
      'Commander rules: Starting life 40 · 1 free mulligan · Commander tax 2 · Commander damage 21 · Singleton decks · Color identity enforced · Decks of 100 cards',
    )
    expect(line).toHaveTextContent('House rules')
    await user.click(screen.getByRole('radio', { name: /^Normal/ }))
    expect(document.querySelector('[data-slot="format-rules"]')).toBeNull()
  })
})
