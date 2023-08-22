use crate::db;
use crate::node::Node;
use anyhow::Context;
use anyhow::Result;
use bitcoin::hashes::hex::ToHex;

pub fn sync(node: Node) -> Result<()> {
    let mut conn = node.pool.get()?;
    let open_and_closing_positions =
        db::positions::Position::get_all_open_or_closing_positions(&mut conn)
            .context("Failed to load open and closing positions")?;

    for position in open_and_closing_positions {
        let temporary_contract_id = match position.temporary_contract_id {
            None => {
                tracing::trace!(position_id=%position.id, "Position does not have temporary contract id, skipping");
                continue;
            }
            Some(temporary_contract_id) => temporary_contract_id,
        };

        let contract = match node.inner.get_closed_contract(temporary_contract_id) {
            Ok(Some(closed_contract)) => closed_contract,
            Ok(None) => {
                tracing::trace!(position_id=%position.id, "Position not closed yet, skipping");
                continue;
            }
            Err(e) => {
                tracing::error!(position_id=%position.id, "Failed to get closed contract from DLC manager storage: {e:#}");
                continue;
            }
        };

        tracing::debug!(
            ?position,
            "Setting position to closed to match the contract state."
        );

        if let Err(e) =
            db::positions::Position::set_position_to_closed(&mut conn, position.id, contract.pnl)
        {
            tracing::error!(position_id=%position.id, temporary_contract_id=%temporary_contract_id.to_hex(), pnl=%contract.pnl, "Failed to set position to closed: {e:#}")
        }
    }

    Ok(())
}
