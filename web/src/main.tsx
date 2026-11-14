import { createRouter, RouterProvider } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import { AuthProvider } from './features/auth/AuthProvider'
import { routeTree } from './routeTree.gen'
import { ROUTER_OPTIONS } from './routerOptions'

const router = createRouter({ routeTree, ...ROUTER_OPTIONS })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

// Dev-only visual directions (`?look=a|b|c`, docs/ui-audit.md); the production build drops this branch.
if (import.meta.env.DEV) void import('./dev/looks/look').then((looks) => looks.installLook())

const container = document.getElementById('root')
if (!container) throw new Error('missing #root element')

createRoot(container).render(
  <StrictMode>
    <AuthProvider>
      <RouterProvider router={router} />
    </AuthProvider>
  </StrictMode>,
)
