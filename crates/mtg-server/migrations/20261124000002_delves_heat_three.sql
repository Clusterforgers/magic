-- Heat runs from 0 to 3 (Legendary): the column accepts every Heat the campaign offers.
ALTER TABLE delves DROP CONSTRAINT delves_heat_check;
ALTER TABLE delves ADD CONSTRAINT delves_heat_check CHECK (heat BETWEEN 0 AND 3);
