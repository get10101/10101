// @generated automatically by Diesel CLI.

diesel::table! {
    last_login (id) {
        id -> Nullable<Integer>,
        date -> Text,
    }
}
