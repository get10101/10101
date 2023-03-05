use crate::schema;
use crate::schema::last_login;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Integer;
use diesel::sql_types::Text;
use diesel::Queryable;
use time::format_description;
use time::OffsetDateTime;

const SQLITE_DATETIME_FMT: &str = "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour \
         sign:mandatory]:[offset_minute]:[offset_second]";

#[derive(Queryable, QueryableByName, Debug, Clone)]
#[diesel(table_name = last_login)]
pub struct LastLogin {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub date: String,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = last_login)]
pub struct NewLastLogin {
    #[diesel(sql_type = Integer)]
    pub id: i32,
    #[diesel(sql_type = Text)]
    pub date: String,
}

impl LastLogin {
    /// Updates the timestamp the user logged in for the last time, returns the one before if
    /// successful
    pub fn update_last_login(
        last_login: OffsetDateTime,
        conn: &mut SqliteConnection,
    ) -> QueryResult<LastLogin> {
        let old_login = sql_query(
            "SELECT
                    id, date
                    FROM
                    last_login order by id",
        )
        .load::<LastLogin>(conn)?;
        let maybe_last_login = old_login.get(0).cloned();

        let format = format_description::parse(SQLITE_DATETIME_FMT).unwrap();

        let date = last_login.format(&format).unwrap();
        diesel::insert_into(last_login::table)
            .values(&NewLastLogin {
                id: 1,
                date: date.clone(),
            })
            .on_conflict(schema::last_login::id)
            .do_update()
            .set(schema::last_login::date.eq(date.clone()))
            .execute(conn)?;

        let login = maybe_last_login.unwrap_or(LastLogin { id: 1, date });
        Ok(login)
    }
}

#[cfg(test)]
pub mod test {
    use crate::db::models::LastLogin;
    use crate::db::MIGRATIONS;
    use diesel::Connection;
    use diesel::SqliteConnection;
    use diesel_migrations::MigrationHarness;
    use time::OffsetDateTime;

    #[test]
    pub fn when_no_login_return_input() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let back_in_the_days = OffsetDateTime::UNIX_EPOCH;
        let last_login = LastLogin::update_last_login(back_in_the_days, &mut connection).unwrap();

        assert_eq!("1970-01-01 00:00:00 +00:00:00".to_string(), last_login.date);
    }

    #[test]
    pub fn when_already_logged_in_return_former_login() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        let back_in_the_days = OffsetDateTime::UNIX_EPOCH;
        let _ = LastLogin::update_last_login(back_in_the_days, &mut connection).unwrap();

        let back_in_the_days_as_well_but_10_secs_later =
            OffsetDateTime::from_unix_timestamp(back_in_the_days.unix_timestamp() + 10).unwrap();
        let _ = LastLogin::update_last_login(
            back_in_the_days_as_well_but_10_secs_later,
            &mut connection,
        )
        .unwrap();

        let now = OffsetDateTime::now_utc();
        let last_login = LastLogin::update_last_login(now, &mut connection).unwrap();

        assert_eq!("1970-01-01 00:00:10 +00:00:00".to_string(), last_login.date);
    }
}
