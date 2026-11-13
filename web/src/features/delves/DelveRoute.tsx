/** The route component of `/delves/$delveId`: the run page for the id of the path. */
import { getRouteApi } from '@tanstack/react-router'
import { DelvePage } from './DelvePage'

const route = getRouteApi('/delves_/$delveId')

export function DelveRoute() {
  const { delveId } = route.useParams()
  return <DelvePage id={delveId} />
}
