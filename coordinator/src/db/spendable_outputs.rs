use crate::schema::spendable_outputs;
use anyhow::anyhow;
use anyhow::Result;
use autometrics::autometrics;
use bitcoin::hashes::hex::FromHex;
use bitcoin::hashes::hex::ToHex;
use diesel::prelude::*;
use lightning::chain::keysinterface::DelayedPaymentOutputDescriptor;
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::chain::keysinterface::StaticPaymentOutputDescriptor;
use lightning::chain::transaction::OutPoint;
use lightning::util::ser::Readable;
use lightning::util::ser::Writeable;

#[autometrics]
pub(crate) fn insert(
    conn: &mut PgConnection,
    output: SpendableOutputDescriptor,
) -> QueryResult<()> {
    diesel::insert_into(spendable_outputs::table)
        .values(NewSpendableOutput::from(output))
        .execute(conn)?;

    Ok(())
}

#[autometrics]
pub fn get(
    conn: &mut PgConnection,
    outpoint: &OutPoint,
) -> Result<Option<SpendableOutputDescriptor>> {
    let output: Option<SpendableOutput> = spendable_outputs::table
        .filter(spendable_outputs::txid.eq(outpoint.txid.to_string()))
        .first(conn)
        .optional()?;

    let output = output
        .map(|output| anyhow::Ok(output.try_into()?))
        .transpose()?;

    Ok(output)
}

#[autometrics]
pub fn get_all(conn: &mut PgConnection) -> Result<Vec<SpendableOutputDescriptor>> {
    let outputs: Vec<SpendableOutput> = spendable_outputs::table.load(conn)?;
    outputs
        .into_iter()
        .map(SpendableOutputDescriptor::try_from)
        .collect()
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = spendable_outputs)]
struct NewSpendableOutput {
    txid: String,
    vout: i32,
    descriptor: String,
}

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = spendable_outputs)]
struct SpendableOutput {
    #[diesel(column_name = "id")]
    _id: i32,
    #[diesel(column_name = "txid")]
    _txid: String,
    #[diesel(column_name = "vout")]
    _vout: i32,
    descriptor: String,
}

impl From<SpendableOutputDescriptor> for NewSpendableOutput {
    fn from(descriptor: SpendableOutputDescriptor) -> Self {
        use SpendableOutputDescriptor::*;
        let outpoint = match &descriptor {
            StaticOutput { outpoint, .. } => outpoint,
            DelayedPaymentOutput(DelayedPaymentOutputDescriptor { outpoint, .. }) => outpoint,
            StaticPaymentOutput(StaticPaymentOutputDescriptor { outpoint, .. }) => outpoint,
        };

        let descriptor = descriptor.encode().to_hex();

        Self {
            txid: outpoint.txid.to_string(),
            vout: outpoint.index as i32,
            descriptor,
        }
    }
}

impl TryFrom<SpendableOutput> for SpendableOutputDescriptor {
    type Error = anyhow::Error;

    fn try_from(value: SpendableOutput) -> Result<Self, Self::Error> {
        let bytes = Vec::from_hex(&value.descriptor)?;
        let descriptor = Self::read(&mut lightning::io::Cursor::new(bytes))
            .map_err(|e| anyhow!("Failed to decode spendable output descriptor: {e}"))?;

        Ok(descriptor)
    }
}
