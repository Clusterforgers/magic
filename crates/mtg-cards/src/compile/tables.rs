//! The table of a die roll: "When this creature enters, roll a d20." and the lines after it ("1—9 | S", "20 | S") are one
//! ability; each row is read as a spell and joins the table of the roll the clause before it ends in. A table is read
//! whole: when one row does not read, the roll and its rows stay with the table.

use super::Pending;
use super::read;

/// Whether the clause is a line of a table that nothing has read yet.
fn is_open_row(clause: &Pending) -> bool {
    !clause.read && clause.ability.is_none() && read::die_row(&clause.text).is_some()
}

/// Reads the rows after each clause that ends in a die roll into its table.
pub(super) fn attach(pending: &mut [Pending], x_free: bool, own: &read::Own) {
    let mut head = 0;
    while head < pending.len() {
        let rows = pending[head + 1..]
            .iter()
            .take_while(|clause| is_open_row(clause))
            .count();
        let rolls = pending[head]
            .ability
            .as_ref()
            .is_some_and(read::ends_in_roll);
        if rows > 0 && rolls {
            let table = table(
                &pending[head],
                &pending[head + 1..=head + rows],
                x_free,
                own,
            );
            if table.is_some() {
                pending[head].ability = table;
                pending[head + 1..=head + rows]
                    .iter_mut()
                    .for_each(|row| row.read = true);
            } else {
                pending[head].ability = None;
                pending[head].read = false;
            }
        }
        head += 1 + rows;
    }
}

/// The ability of the head with the rows in its table, when every row reads and follows the rows before it.
fn table(
    head: &Pending,
    rows: &[Pending],
    x_free: bool,
    own: &read::Own,
) -> Option<crate::ability::Ability> {
    let mut ability = head.ability.clone()?;
    for row in rows {
        let found = read::die_row_of(&row.text, x_free, own)?;
        ability = read::add_die_row(ability, found)?;
    }
    Some(ability)
}
