use crate::db::user;
use crate::logger::init_tracing_for_test;
use crate::orderbook::tests::setup_db;
use crate::orderbook::tests::start_postgres;
use bitcoin::secp256k1::PublicKey;
use std::str::FromStr;
use testcontainers::clients::Cli;

#[tokio::test]
async fn registered_user_is_stored_in_db() {
    init_tracing_for_test();

    let docker = Cli::default();
    let (_container, conn_spec) = start_postgres(&docker).unwrap();

    let mut conn = setup_db(conn_spec);

    let users = user::all(&mut conn).unwrap();
    assert!(users.is_empty());

    let dummy_pubkey = dummy_public_key();
    let dummy_email = "dummy@user.com".to_string();
    let nickname = Some("dummy_user".to_string());
    let fcm_token = "just_a_token".to_string();
    let version = Some("1.9.0".to_string());

    let user = user::upsert_user(
        &mut conn,
        dummy_pubkey,
        Some(dummy_email.clone()),
        nickname.clone(),
        version.clone(),
        Some("code1".to_string()),
    )
    .unwrap();
    assert!(user.id.is_some(), "Id should be filled in by diesel");
    user::login_user(&mut conn, dummy_pubkey, fcm_token.clone(), version.clone()).unwrap();

    let users = user::all(&mut conn).unwrap();
    assert_eq!(users.len(), 1);
    // We started without the id, so we can't compare the whole user.
    assert_eq!(users.first().unwrap().pubkey, dummy_pubkey.to_string());
    assert_eq!(users.first().unwrap().contact, dummy_email);
    assert_eq!(users.first().unwrap().nickname, nickname);
    assert_eq!(users.first().unwrap().fcm_token, fcm_token);
    assert_eq!(users.first().unwrap().version, version);
}

fn dummy_public_key() -> PublicKey {
    PublicKey::from_str("02bd998ebd176715fe92b7467cf6b1df8023950a4dd911db4c94dfc89cc9f5a655")
        .unwrap()
}
