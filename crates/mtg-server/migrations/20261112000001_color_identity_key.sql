-- The house rule that lifts the color identity check is spelled `color_identity`, as on the wire.
-- Stored recipes that use the older key `colour_identity` take the new one with the same value.
UPDATE games
SET recipe = jsonb_set(
    recipe #- '{house_rules,deck,colour_identity}',
    '{house_rules,deck,color_identity}',
    recipe #> '{house_rules,deck,colour_identity}'
)
WHERE recipe #> '{house_rules,deck}' ? 'colour_identity';
