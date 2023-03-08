// @generated automatically by Diesel CLI.

diesel::table! {
    orders (id) {
        id -> Int4,
        price -> Float4,
        maker_id -> Text,
        taken -> Bool,
    }
}
