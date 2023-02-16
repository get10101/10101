// @generated automatically by Diesel CLI.

diesel::table! {
    orders (id) {
        id -> Int4,
        price -> Int4,
        maker_id -> Text,
        taken -> Bool,
    }
}
