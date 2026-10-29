import { QueryClient } from '@tanstack/react-query'

/**
 * The query client of the fix marks. The hooks name it themselves, so the button works anywhere a card is shown without
 * a provider above it; tests clear it between cases.
 */
export const fixMarksClient = new QueryClient({
  defaultOptions: { queries: { retry: false, staleTime: 60_000, refetchOnWindowFocus: false } },
})
