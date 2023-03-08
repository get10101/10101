use crate::schema::orders;
use crate::schema::sql_types::DirectionType;
use diesel::prelude::*;
use diesel::result::QueryResult;
use diesel::AsExpression;
use diesel::FromSqlRow;
use diesel::PgConnection;

#[derive(Debug, Clone, Copy, PartialEq, FromSqlRow, AsExpression, Eq)]
#[diesel(sql_type = DirectionType)]
pub enum Direction {
    Long,
    Short,
}

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

#[derive(Queryable, Debug, Clone)]
pub struct Order {
    pub id: i32,
    pub price: f32,
    pub maker_id: String,
    pub taken: bool,
    pub direction: Direction,
    pub quantity: f32,
}

#[derive(Insertable, Debug, PartialEq)]
#[diesel(table_name = orders)]
pub struct NewOrder {
    pub price: f32,
    pub maker_id: String,
    pub taken: bool,
    pub direction: Direction,
    pub quantity: f32,
}

impl Order {
    pub fn all(conn: &mut PgConnection) -> QueryResult<Vec<Order>> {
        orders::dsl::orders.load::<Order>(conn)
    }

    /// Returns the number of affected rows: 1.
    pub fn insert(conn: &mut PgConnection, new_order: NewOrder) -> QueryResult<Order> {
        diesel::insert_into(orders::table)
            .values(&new_order)
            .get_result(conn)
    }

    /// Returns the number of affected rows: 1.
    pub fn update(conn: &mut PgConnection, id: i32, is_taken: bool) -> QueryResult<Order> {
        diesel::update(orders::table)
            .filter(orders::id.eq(id))
            .set(orders::taken.eq(is_taken))
            .get_result(conn)
    }

    /// Returns the order by id
    pub fn get_with_id(conn: &mut PgConnection, uid: i32) -> QueryResult<Option<Order>> {
        let x = orders::table
            .filter(orders::id.eq(uid))
            .load::<Order>(conn)
            .unwrap();

        let option = x.get(0).cloned();
        Ok(option)
    }

    /// Returns the number of affected rows: 1.
    pub fn delete_with_id(conn: &mut PgConnection, order_id: i32) -> QueryResult<usize> {
        diesel::delete(orders::table)
            .filter(orders::id.eq(order_id))
            .execute(conn)
    }

    /// Returns the number of affected rows.
    #[cfg(test)]
    pub fn delete_all(conn: &mut PgConnection) -> QueryResult<usize> {
        diesel::delete(orders::table).execute(conn)
    }
}
