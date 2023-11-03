use crate::schema;
use crate::schema::payments;
use crate::schema::sql_types::HtlcStatusType;
use crate::schema::sql_types::PaymentFlowType;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use bitcoin::hashes::hex::FromHex;
use bitcoin::hashes::hex::ToHex;
use diesel;
use diesel::prelude::*;
use diesel::query_builder::QueryId;
use diesel::AsExpression;
use diesel::FromSqlRow;
use std::any::TypeId;
use time::OffsetDateTime;

#[derive(Queryable, Debug, Clone)]
#[diesel(table_name = payments)]
pub struct Payment {
    pub id: i32,
    pub payment_hash: String,
    pub preimage: Option<String>,
    pub secret: Option<String>,
    pub htlc_status: HtlcStatus,
    pub amount_msat: Option<i64>,
    pub flow: PaymentFlow,
    pub payment_timestamp: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub description: String,
    pub invoice: Option<String>,
    pub fee_msat: Option<i64>,
}

pub fn get(
    payment_hash: lightning::ln::PaymentHash,
    conn: &mut PgConnection,
) -> Result<Option<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)>> {
    let payment_hash = payment_hash.0.to_hex();
    let payment = payments::table
        .filter(payments::payment_hash.eq(payment_hash))
        .first::<Payment>(conn)
        .optional()?;

    let payment = match payment {
        None => None,
        Some(payment) => Some(payment.try_into()?),
    };

    Ok(payment)
}

pub fn get_all(
    conn: &mut PgConnection,
) -> Result<Vec<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)>> {
    let raw_payments = payments::table.load::<Payment>(conn)?;

    let mut payments = Vec::new();
    for raw_payment in raw_payments.into_iter() {
        let raw_payment_hash = raw_payment.payment_hash.clone();
        let payment = raw_payment.try_into();
        match payment {
            Ok(payment) => payments.push(payment),
            Err(e) => {
                // We don't exit in case we cannot load a single payment but only log an error
                tracing::error!(payment_hash=%raw_payment_hash, "Unable to load payment from database, skipping: {e:#}")
            }
        }
    }

    Ok(payments)
}

impl From<(lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)> for NewPayment {
    fn from((payment_hash, info): (lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo)) -> Self {
        Self {
            payment_hash: payment_hash.0.to_hex(),
            preimage: info.preimage.map(|preimage| preimage.0.to_hex()),
            secret: info.secret.map(|secret| secret.0.to_hex()),
            htlc_status: info.status.into(),
            amount_msat: info.amt_msat.to_inner().map(|amt| amt as i64),
            flow: info.flow.into(),
            payment_timestamp: info.timestamp,
            description: info.description,
            invoice: info.invoice,
        }
    }
}

impl TryFrom<Payment> for (lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo) {
    type Error = anyhow::Error;

    fn try_from(value: Payment) -> Result<Self> {
        let payment_hash =
            lightning::ln::PaymentHash(<[u8; 32]>::from_hex(value.payment_hash.as_str())?);

        let preimage = value
            .preimage
            .map(|preimage| {
                anyhow::Ok(lightning::ln::PaymentPreimage(<[u8; 32]>::from_hex(
                    preimage.as_str(),
                )?))
            })
            .transpose()?;

        let secret = value
            .secret
            .map(|secret| {
                anyhow::Ok(lightning::ln::PaymentSecret(<[u8; 32]>::from_hex(
                    secret.as_str(),
                )?))
            })
            .transpose()?;

        let amt_msat =
            ln_dlc_node::MillisatAmount::new(value.amount_msat.map(|amount| amount as u64));
        let fee_msat = ln_dlc_node::MillisatAmount::new(value.fee_msat.map(|amount| amount as u64));

        Ok((
            payment_hash,
            ln_dlc_node::PaymentInfo {
                preimage,
                secret,
                status: value.htlc_status.into(),
                amt_msat,
                fee_msat,
                flow: value.flow.into(),
                timestamp: value.payment_timestamp,
                description: value.description,
                invoice: value.invoice,
                funding_txid: None,
            },
        ))
    }
}

impl From<HtlcStatus> for ln_dlc_node::HTLCStatus {
    fn from(value: HtlcStatus) -> Self {
        match value {
            HtlcStatus::Pending => Self::Pending,
            HtlcStatus::Succeeded => Self::Succeeded,
            HtlcStatus::Failed => Self::Failed,
        }
    }
}

impl From<ln_dlc_node::PaymentFlow> for PaymentFlow {
    fn from(value: ln_dlc_node::PaymentFlow) -> Self {
        match value {
            ln_dlc_node::PaymentFlow::Inbound => Self::Inbound,
            ln_dlc_node::PaymentFlow::Outbound => Self::Outbound,
        }
    }
}

impl From<PaymentFlow> for ln_dlc_node::PaymentFlow {
    fn from(value: PaymentFlow) -> Self {
        match value {
            PaymentFlow::Inbound => Self::Inbound,
            PaymentFlow::Outbound => Self::Outbound,
        }
    }
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = payments)]
pub(crate) struct NewPayment {
    #[diesel(sql_type = Text)]
    pub payment_hash: String,
    #[diesel(sql_type = Nullabel<Text>)]
    pub preimage: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    pub secret: Option<String>,
    pub htlc_status: HtlcStatus,
    #[diesel(sql_type = Nullable<BigInt>)]
    pub amount_msat: Option<i64>,
    pub flow: PaymentFlow,
    pub payment_timestamp: OffsetDateTime,
    #[diesel(sql_type = Text)]
    pub description: String,
    #[diesel(sql_type = Nullable<Text>)]
    pub invoice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = HtlcStatusType)]
pub enum HtlcStatus {
    Pending,
    Succeeded,
    Failed,
}

impl QueryId for HtlcStatus {
    type QueryId = HtlcStatusType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

impl From<ln_dlc_node::HTLCStatus> for HtlcStatus {
    fn from(value: ln_dlc_node::HTLCStatus) -> Self {
        match value {
            ln_dlc_node::HTLCStatus::Pending => HtlcStatus::Pending,
            ln_dlc_node::HTLCStatus::Succeeded => HtlcStatus::Succeeded,
            ln_dlc_node::HTLCStatus::Failed => HtlcStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression)]
#[diesel(sql_type = PaymentFlowType)]
pub enum PaymentFlow {
    Inbound,
    Outbound,
}

impl QueryId for PaymentFlow {
    type QueryId = PaymentFlowType;
    const HAS_STATIC_QUERY_ID: bool = false;

    fn query_id() -> Option<TypeId> {
        None
    }
}

pub fn insert(
    payment: (lightning::ln::PaymentHash, ln_dlc_node::PaymentInfo),
    conn: &mut PgConnection,
) -> Result<()> {
    let payment: NewPayment = payment.into();
    let affected_rows = diesel::insert_into(payments::table)
        .values(&payment)
        .execute(conn)?;

    ensure!(affected_rows > 0, "Could not insert payment");

    Ok(())
}

pub fn update(
    payment_hash: lightning::ln::PaymentHash,
    htlc_status: ln_dlc_node::HTLCStatus,
    amount_msat: ln_dlc_node::MillisatAmount,
    fee_msat: ln_dlc_node::MillisatAmount,
    preimage: Option<lightning::ln::PaymentPreimage>,
    secret: Option<lightning::ln::PaymentSecret>,
    conn: &mut PgConnection,
) -> Result<OffsetDateTime> {
    let updated_at = OffsetDateTime::now_utc();

    let preimage = preimage.map(|preimage| preimage.0.to_hex());
    let secret = secret.map(|secret| secret.0.to_hex());

    let payment_hash = payment_hash.0.to_hex();
    let htlc_status: HtlcStatus = htlc_status.into();
    let amount_msat = amount_msat.to_inner().map(|amt| amt as i64);
    let fee_msat = fee_msat.to_inner().map(|amt| amt as i64);

    conn.transaction::<(), _, _>(|conn| {
        let affected_rows = diesel::update(payments::table)
            .filter(schema::payments::payment_hash.eq(&payment_hash))
            .set(schema::payments::htlc_status.eq(htlc_status))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update payment HTLC status")
        }

        if let Some(amount_msat) = amount_msat {
            let affected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::amount_msat.eq(amount_msat))
                .execute(conn)?;

            if affected_rows == 0 {
                bail!("Could not update payment amount")
            }
        }

        if let Some(fee_msat) = fee_msat {
            let affected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::fee_msat.eq(fee_msat))
                .execute(conn)?;

            if affected_rows == 0 {
                bail!("Could not update payment fee amount")
            }
        }

        if let Some(preimage) = preimage {
            let affected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::preimage.eq(preimage))
                .execute(conn)?;

            if affected_rows == 0 {
                bail!("Could not update payment preimage")
            }
        }

        if let Some(secret) = secret {
            let affected_rows = diesel::update(payments::table)
                .filter(schema::payments::payment_hash.eq(&payment_hash))
                .set(schema::payments::secret.eq(secret))
                .execute(conn)?;

            if affected_rows == 0 {
                bail!("Could not update payment secret")
            }
        }

        let affected_rows = diesel::update(payments::table)
            .filter(schema::payments::payment_hash.eq(&payment_hash))
            .set(schema::payments::updated_at.eq(updated_at))
            .execute(conn)?;

        if affected_rows == 0 {
            bail!("Could not update payment updated_at xtimestamp")
        }

        Ok(())
    })?;

    Ok(updated_at)
}
