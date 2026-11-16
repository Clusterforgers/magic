// Goal: an alert with an icon keeps the icon in a column of its own width: every other part, a button row too, stands in the second column.
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Alert, AlertDescription, AlertTitle } from './alert'

describe('Alert', () => {
  it('puts every part but the icon in the second column', () => {
    render(
      <Alert variant="destructive">
        <svg aria-hidden="true" />
        <AlertTitle>Cannot load</AlertTitle>
        <AlertDescription>The server is restarting.</AlertDescription>
        <div>
          <button type="button">Retry</button>
        </div>
      </Alert>,
    )
    const alert = screen.getByRole('alert')
    expect(alert.className).toContain('has-[>svg]:[&>:not(svg)]:col-start-2')
    expect(alert.className).toContain('has-[>svg]:grid-cols-[auto_1fr]')
  })
})
