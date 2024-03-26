use crate::db;
use crate::db::user::User;
use anyhow::ensure;
use anyhow::Context;
use anyhow::Result;
use bitcoin::secp256k1::PublicKey;
use diesel::PgConnection;
use semver::Comparator;
use semver::Op;
use semver::Version;
use semver::VersionReq;

pub fn check_version(conn: &mut PgConnection, trader_id: &PublicKey) -> Result<()> {
    let user: User = db::user::get_user(conn, trader_id)?.context("Couldn't find user")?;

    let app_version = user.version.context("No version found")?;
    let app_version = Version::parse(app_version.as_str())?;

    let coordinator_version = env!("CARGO_PKG_VERSION").to_string();
    let coordinator_version = Version::parse(coordinator_version.as_str())?;

    check_compatibility(app_version, coordinator_version)?;

    Ok(())
}

fn check_compatibility(app_version: Version, coordinator_version: Version) -> Result<()> {
    let req = VersionReq {
        comparators: vec![
            Comparator {
                op: Op::Exact,
                major: coordinator_version.major,
                minor: Some(coordinator_version.minor),
                patch: None,
                pre: Default::default(),
            },
            // We introduced sematic versioning only after this version, hence, we say each app
            // needs to have at least this version.
            Comparator {
                op: Op::GreaterEq,
                major: 2,
                minor: Some(0),
                patch: Some(6),
                pre: Default::default(),
            },
        ],
    };

    ensure!(
        req.matches(&app_version),
        format!("Please upgrade to the latest version: {coordinator_version}")
    );
    Ok(())
}

#[cfg(test)]
pub mod tests {
    use crate::check_version::check_compatibility;
    use semver::Version;

    #[test]
    pub fn same_versions_are_compatible() {
        let app_version = Version::new(2, 1, 1);
        let coordinator_version = Version::new(2, 1, 1);

        assert!(check_compatibility(app_version, coordinator_version).is_ok());
    }

    #[test]
    pub fn same_minor_and_major_are_compatible() {
        let app_version = Version::new(2, 1, 1);
        let coordinator_version = Version::new(2, 1, 3);

        assert!(check_compatibility(app_version, coordinator_version).is_ok());
    }

    #[test]
    pub fn different_minor_are_incompatible() {
        let app_version = Version::new(2, 1, 1);
        let coordinator_version = Version::new(2, 2, 1);

        assert!(check_compatibility(app_version, coordinator_version).is_err());
    }

    #[test]
    pub fn different_major_are_incompatible() {
        let app_version = Version::new(2, 1, 1);
        let coordinator_version = Version::new(3, 1, 1);

        assert!(check_compatibility(app_version, coordinator_version).is_err());
    }

    #[test]
    pub fn if_version_smaller_than_2_0_6_then_error() {
        let app_version = Version::new(2, 0, 5);
        let coordinator_version = Version::new(2, 0, 7);

        assert!(check_compatibility(app_version, coordinator_version).is_err());
    }

    #[test]
    pub fn if_version_greater_equal_than_2_0_6_then_ok() {
        let app_version = Version::new(2, 0, 6);
        let coordinator_version = Version::new(2, 0, 7);

        assert!(check_compatibility(app_version, coordinator_version).is_ok());
    }
}
