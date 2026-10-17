import type { Fetch } from '@/net/api'

/** One request the client made. */
export interface FetchCall {
  url: string
  init: RequestInit | undefined
}

/** A JSON response with the given status. */
export function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } })
}

/** A `fetch` that answers every request with `answer` and records what it was asked. */
export function fakeFetch(answer: (call: FetchCall) => Response | Promise<Response>): {
  fetch: Fetch
  calls: FetchCall[]
} {
  const calls: FetchCall[] = []
  return {
    calls,
    fetch: async (url, init) => {
      const call = { url, init }
      calls.push(call)
      return answer(call)
    },
  }
}
