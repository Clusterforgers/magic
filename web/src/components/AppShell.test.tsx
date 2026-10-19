// Goal: the shell frames every page with the brand, a main navigation that marks the current page and offers "My games" only to an account, a theme menu that changes the page's theme and remembers it, a way past the header by keyboard, and a notice while the browser is offline.
import { act, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { appTheme } from '@/hooks/useTheme'
import { listed } from '@/test-utils/accountsServer'
import { renderApp } from '@/test-utils/appHarness'

beforeEach(() => {
  appTheme().setChoice('system')
})

afterEach(() => {
  appTheme().setChoice('system')
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const ADA = { username: 'Ada_L', password: 'correct horse battery' }
const mainNav = () => screen.getByRole('navigation', { name: 'Main' })

async function open(url = '/', signedIn = false) {
  const app = await renderApp(url, {
    accounts: {
      users: { [ADA.username]: ADA.password },
      signedIn: signedIn ? ADA.username : undefined,
      games: [listed()],
    },
  })
  await waitFor(() => expect(app.store.getState().status).not.toBe('loading'))
  return app
}

describe('the main navigation', () => {
  it('has the brand, Play and Create for a guest, and no My games', async () => {
    await open()
    expect(screen.getByRole('link', { name: 'Magic' })).toHaveAttribute('href', '/')
    const links = within(mainNav()).getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual(['Play', 'Create'])
    expect(links.map((link) => link.getAttribute('href'))).toEqual(['/', '/'])
  })

  it('adds My games, between Play and Create, for an account', async () => {
    await open('/', true)
    const links = within(mainNav()).getAllByRole('link')
    expect(links.map((link) => link.textContent)).toEqual(['Play', 'My games', 'Create'])
    expect(links.map((link) => link.getAttribute('href'))).toEqual(['/', '/games', '/'])
  })

  it('marks the page you are on, and only that one', async () => {
    const app = await open('/games', true)
    await screen.findByRole('heading', { name: 'My games' })
    expect(within(mainNav()).getByRole('link', { name: 'My games' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).not.toHaveAttribute('aria-current')
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Play' }))
    await waitFor(() => expect(app.router.state.location.pathname).toBe('/'))
    expect(within(mainNav()).getByRole('link', { name: 'Play' })).toHaveAttribute('aria-current', 'page')
    expect(within(mainNav()).getByRole('link', { name: 'My games' })).not.toHaveAttribute('aria-current')
  })

  it('opens the page of a link without a reload', async () => {
    const app = await open('/games', true)
    await screen.findByRole('heading', { name: 'My games' })
    await app.user.click(within(mainNav()).getByRole('link', { name: 'Create' }))
    expect(await screen.findByRole('form', { name: 'Create a game' })).toBeInTheDocument()
  })
})

describe('the way past the header', () => {
  it('is a first stop that moves the focus into the page, without touching the address', async () => {
    const app = await open('/')
    await screen.findByRole('form', { name: 'Create a game' })
    await app.user.tab()
    const skip = screen.getByRole('button', { name: 'Skip to content' })
    expect(skip).toHaveFocus()
    const before = app.router.state.location.href
    await app.user.keyboard('{Enter}')
    expect(screen.getByRole('main')).toHaveFocus()
    expect(app.router.state.location.href).toBe(before)
  })
})

describe('the theme menu', () => {
  it('shows the choice in force and offers Light, Dark and System', async () => {
    const app = await open()
    await app.user.click(screen.getByRole('button', { name: 'Theme: System' }))
    const items = await screen.findAllByRole('menuitemradio')
    expect(items.map((item) => item.textContent)).toEqual(['Light', 'Dark', 'System'])
    expect(items.map((item) => item.getAttribute('aria-checked'))).toEqual(['false', 'false', 'true'])
  })

  it('puts the page in the dark theme, remembers it and shows it on the button', async () => {
    const app = await open()
    expect(document.documentElement).not.toHaveClass('dark')
    await app.user.click(screen.getByRole('button', { name: 'Theme: System' }))
    await app.user.click(await screen.findByRole('menuitemradio', { name: 'Dark' }))
    expect(document.documentElement).toHaveClass('dark')
    expect(window.localStorage.getItem('theme')).toBe('dark')
    expect(screen.getByRole('button', { name: 'Theme: Dark' })).toBeInTheDocument()
  })

  it('goes back to the system, which forgets the choice', async () => {
    const app = await open()
    appTheme().setChoice('dark')
    await app.user.click(await screen.findByRole('button', { name: 'Theme: Dark' }))
    await app.user.click(await screen.findByRole('menuitemradio', { name: 'System' }))
    expect(document.documentElement).not.toHaveClass('dark')
    expect(window.localStorage.getItem('theme')).toBeNull()
  })

  it('is operable from the keyboard', async () => {
    const app = await open()
    screen.getByRole('button', { name: 'Theme: System' }).focus()
    await app.user.keyboard('{Enter}')
    const light = await screen.findByRole('menuitemradio', { name: 'Light' })
    await act(async () => light.focus())
    await app.user.keyboard('{Enter}')
    expect(appTheme().getChoice()).toBe('light')
    expect(await screen.findByRole('button', { name: 'Theme: Light' })).toHaveFocus()
  })
})

describe('while the browser is offline', () => {
  it('says so politely, and stops saying so when the network is back', async () => {
    const onLine = vi.spyOn(window.navigator, 'onLine', 'get').mockReturnValue(false)
    await open()
    expect(screen.getByRole('status')).toHaveTextContent('You are offline')
    onLine.mockReturnValue(true)
    act(() => {
      window.dispatchEvent(new Event('online'))
    })
    await waitFor(() => expect(screen.queryByText('You are offline')).toBeNull())
  })

  it('is silent while the browser is online', async () => {
    await open()
    expect(screen.queryByText('You are offline')).toBeNull()
  })
})
