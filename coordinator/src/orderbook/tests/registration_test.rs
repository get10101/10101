use crate::db::user;
use crate::db::user::User;
use crate::orderbook::tests::init_tracing;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use bitcoin::secp256k1::PublicKey;
use coordinator_commons::RegisterParams;
use std::str::FromStr;
use testcontainers::clients::Cli;

#[tokio::test]
async fn registered_user_is_stored_in_db() {
    init_tracing();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let users = user::all(&mut conn).unwrap();
    assert!(users.is_empty());

    let dummy_user: User = RegisterParams {
        pubkey: dummy_public_key(),
        email: Some("dummy@user.com".to_string()),
        nostr: None,
    }
    .into();

    let user = user::insert(&mut conn, dummy_user.clone()).unwrap();
    assert!(user.id.is_some(), "Id should be filled in by diesel");

    let users = user::all(&mut conn).unwrap();
    assert_eq!(users.len(), 1);
    // We started without the id, so we can't compare the whole user.
    assert_eq!(users.first().unwrap().pubkey, dummy_user.pubkey);
    assert_eq!(users.first().unwrap().email, dummy_user.email);
    assert_eq!(users.first().unwrap().nostr, dummy_user.nostr);
}

fn dummy_public_key() -> PublicKey {
    PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
        .unwrap()
}
