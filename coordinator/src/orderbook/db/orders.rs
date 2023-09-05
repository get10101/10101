use crate::orderbook::db::custom_types::Direction;
use crate::orderbook::db::custom_types::OrderType;
use crate::schema::orders;
use diesel::prelude::*;
use diesel::result::QueryResult;
use diesel::PgConnection;
use orderbook_commons::NewOrder as OrderbookNewOrder;
use orderbook_commons::Order as OrderbookOrder;
use orderbook_commons::OrderType as OrderBookOrderType;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::OffsetDateTime;
use trade::Direction as OrderbookDirection;
use uuid::Uuid;

impl From<trade::Direction> for Direction {
    fn from(value: trade::Direction) -> Self {
        match value {
            trade::Direction::Long => Direction::Long,
            trade::Direction::Short => Direction::Short,
        }
    }
}

impl From<Direction> for trade::Direction {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Long => trade::Direction::Long,
            Direction::Short => trade::Direction::Short,
        }
    }
}

impl From<OrderType> for OrderBookOrderType {
    fn from(value: OrderType) -> Self {
        match value {
            OrderType::Market => OrderBookOrderType::Market,
            OrderType::Limit => OrderBookOrderType::Limit,
        }
    }
}

impl From<OrderBookOrderType> for OrderType {
    fn from(value: OrderBookOrderType) -> Self {
        match value {
            OrderBookOrderType::Market => OrderType::Market,
            OrderBookOrderType::Limit => OrderType::Limit,
        }
    }
}

#[derive(Queryable, Debug, Clone)]
struct Order {
    // this id is only internally but needs to be here or diesel complains
    #[allow(dead_code)]
    pub id: i32,
    pub trader_order_id: Uuid,
    pub price: f32,
    pub trader_id: String,
    pub taken: bool,
    pub direction: Direction,
    pub quantity: f32,
    pub timestamp: OffsetDateTime,
    pub order_type: OrderType,
    pub expiry: OffsetDateTime,
    pub position_expiry: OffsetDateTime,
}

impl From<Order> for OrderbookOrder {
    fn from(value: Order) -> Self {
        OrderbookOrder {
            id: value.trader_order_id,
            price: Decimal::from_f32(value.price).expect("To be able to convert f32 to decimal"),
            trader_id: value.trader_id.parse().expect("to have a valid pubkey"),
            taken: value.taken,
            direction: value.direction.into(),
            quantity: Decimal::from_f32(value.quantity)
                .expect("To be able to convert f32 to decimal"),
            order_type: value.order_type.into(),
            timestamp: value.timestamp,
            expiry: value.expiry,
            position_expiry: value.position_expiry,
        }
    }
}

#[derive(Insertable, Debug, PartialEq)]
#[diesel(table_name = orders)]
struct NewOrder {
    pub trader_order_id: Uuid,
    pub price: f32,
    pub trader_id: String,
    pub taken: bool,
    pub direction: Direction,
    pub quantity: f32,
    pub order_type: OrderType,
    pub expiry: OffsetDateTime,
    pub position_expiry: OffsetDateTime,
}

impl From<OrderbookNewOrder> for NewOrder {
    fn from(value: OrderbookNewOrder) -> Self {
        NewOrder {
            trader_order_id: value.id,
            price: value
                .price
                .round_dp(2)
                .to_f32()
                .expect("To be able to convert decimal to f32"),
            trader_id: value.trader_id.to_string(),
            taken: false,
            direction: value.direction.into(),
            quantity: value
                .quantity
                .round_dp(2)
                .to_f32()
                .expect("To be able to convert decimal to f32"),
            order_type: value.order_type.into(),
            expiry: value.order_expiry,
            position_expiry: value.position_expiry,
        }
    }
}

pub fn all(conn: &mut PgConnection, show_expired: bool) -> QueryResult<Vec<OrderbookOrder>> {
    let orders: Vec<Order> = if show_expired {
        orders::dsl::orders.load::<Order>(conn)?
    } else {
        orders::table
            .filter(orders::expiry.gt(OffsetDateTime::now_utc()))
            .load::<Order>(conn)?
    };

    Ok(orders.into_iter().map(OrderbookOrder::from).collect())
}

/// Loads all orders by the given order direction and type
pub fn all_by_direction_and_type(
    conn: &mut PgConnection,
    direction: OrderbookDirection,
    order_type: OrderBookOrderType,
    taken: bool,
    filter_expired: bool,
) -> QueryResult<Vec<OrderbookOrder>> {
    let filters = orders::table
        .filter(orders::direction.eq(Direction::from(direction)))
        .filter(orders::order_type.eq(OrderType::from(order_type)))
        .filter(orders::taken.eq(taken));
    let orders: Vec<Order> = if filter_expired {
        filters
            .filter(orders::expiry.gt(OffsetDateTime::now_utc()))
            .load::<Order>(conn)?
    } else {
        filters.load::<Order>(conn)?
    };

    Ok(orders.into_iter().map(OrderbookOrder::from).collect())
}

/// Returns the number of affected rows: 1.
pub fn insert(conn: &mut PgConnection, order: OrderbookNewOrder) -> QueryResult<OrderbookOrder> {
    let order: Order = diesel::insert_into(orders::table)
        .values(NewOrder::from(order))
        .get_result(conn)?;

    Ok(OrderbookOrder::from(order))
}

/// Returns the number of affected rows: 1.
pub fn set_is_taken(
    conn: &mut PgConnection,
    id: Uuid,
    is_taken: bool,
) -> QueryResult<OrderbookOrder> {
    let order: Order = diesel::update(orders::table)
        .filter(orders::trader_order_id.eq(id))
        .set(orders::taken.eq(is_taken))
        .get_result(conn)?;

    Ok(OrderbookOrder::from(order))
}

/// Returns the order by id
pub fn get_with_id(conn: &mut PgConnection, uid: Uuid) -> QueryResult<Option<OrderbookOrder>> {
    let x = orders::table
        .filter(orders::trader_order_id.eq(uid))
        .load::<Order>(conn)?;

    let option = x.get(0).map(|order| OrderbookOrder::from(order.clone()));
    Ok(option)
}

/// Returns the number of affected rows: 1.
pub fn delete_with_id(conn: &mut PgConnection, order_id: Uuid) -> QueryResult<usize> {
    diesel::delete(orders::table)
        .filter(orders::trader_order_id.eq(order_id))
        .execute(conn)
}
