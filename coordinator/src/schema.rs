// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Direction_Type"))]
    pub struct DirectionType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::DirectionType;

    orders (id) {
        id -> Int4,
        price -> Float4,
        maker_id -> Text,
        taken -> Bool,
        direction -> DirectionType,
        quantity -> Float4,
    }
}
