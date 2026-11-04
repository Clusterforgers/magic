/* eslint-disable */
// @ts-nocheck
import { createRoot } from 'react-dom/client'
import './index.css'
import { CardZoom } from '@/features/decks/preview/PreviewCards'
import { CardPreview } from '@/features/builder/CardPreview'
import { CardsApiContext } from '@/features/table/cardsContext'

const text =
  'Flying\nWhen this enters, draw a card, then discard a card at random.\nWhenever you cast a spell, each player loses 1 life.'
const hit = { oracle_id: 'x', name: 'Mock Dragon', mana_cost: '{3}{R}', mana_value: 4, type_line: 'Creature — Dragon', colors: ['R'], color_identity: ['R'], images: {}, support: 'partial' }
const reasons = [
  { face: 0, span: { start: 7, end: 69 }, text: 'Discarding at random is not run.' },
  { face: 0, span: { start: 78, end: 127 }, text: 'Life loss for each player is not run.' },
]
const detail = { card: hit, oracle_text: text, keywords: [], faces: [], legalities: {}, printings: [], rulings: [], reasons }
const api = { get: async () => detail }
const pc = { oracleId: 'x', name: 'Mock Dragon', count: 1, typeLine: hit.type_line, manaValue: 4, manaCost: '{3}{R}', identity: ['R'], support: 'partial', reasons: reasons.map((r) => r.text) }
const zoom = new URLSearchParams(location.search).get('v') === 'zoom'
createRoot(document.getElementById('root')).render(
  <CardsApiContext.Provider value={api}>
    {zoom ? <CardZoom card={pc} onClose={() => {}} /> : <CardPreview card={hit} open api={api} onClose={() => {}} />}
  </CardsApiContext.Provider>,
)
