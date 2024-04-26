use crate::storage::DlcStoreProvider;
use crate::storage::KeyValue;
use anyhow::Result;
use sled::Db;

#[derive(Clone)]
pub struct SledStorageProvider {
    db: Db,
}

pub struct SledStorageExport {
    pub kind: u8,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

impl SledStorageProvider {
    pub fn new(path: &str) -> Self {
        SledStorageProvider {
            db: sled::open(path).expect("valid path"),
        }
    }

    /// Exports all key value pairs from the sled storage
    pub fn export(&self) -> Vec<SledStorageExport> {
        let mut export = vec![];
        for (collection_type, collection_name, collection_iter) in self.db.export() {
            if collection_type != b"tree" {
                continue;
            }
            for mut kv in collection_iter {
                let value = kv.pop().expect("failed to get value from tree export");
                let key = kv.pop().expect("failed to get key from tree export");
                let kind = collection_name
                    .first()
                    .expect("failed to get kind from tree export");

                export.push(SledStorageExport {
                    kind: *kind,
                    key,
                    value,
                });
            }
        }
        export
    }
}

impl DlcStoreProvider for SledStorageProvider {
    fn read(&self, kind: u8, key: Option<Vec<u8>>) -> Result<Vec<KeyValue>> {
        let tree = self.db.open_tree([kind])?;

        if let Some(key) = key {
            let result = match tree.get(key.clone())? {
                Some(value) => vec![KeyValue {
                    key,
                    value: value.to_vec(),
                }],

                None => vec![],
            };

            Ok(result)
        } else {
            let result = tree
                .iter()
                .map(|entry| {
                    let entry = entry.expect("to not fail");
                    KeyValue {
                        key: entry.0.to_vec(),
                        value: entry.1.to_vec(),
                    }
                })
                .collect();

            Ok(result)
        }
    }

    fn write(&self, kind: u8, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.db.open_tree([kind])?.insert(key, value)?;
        self.db.flush()?;
        Ok(())
    }

    fn delete(&self, kind: u8, key: Option<Vec<u8>>) -> Result<()> {
        let tree = self.db.open_tree([kind])?;

        if let Some(key) = key {
            tree.remove(key)?;
        } else {
            tree.clear()?;
        }

        self.db.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::sled::SledStorageProvider;
    use crate::storage::DlcStoreProvider;

    macro_rules! sled_test {
        ($name: ident, $body: expr) => {
            #[test]
            fn $name() {
                let path = format!("{}{}", "test_files/sleddb/", std::stringify!($name));
                {
                    let storage = SledStorageProvider::new(&path);
                    #[allow(clippy::redundant_closure_call)]
                    $body(storage);
                }
                std::fs::remove_dir_all(path).unwrap();
            }
        };
    }

    sled_test!(write_key_and_value, |storage: SledStorageProvider| {
        let result = storage.write(
            1,
            "key".to_string().into_bytes(),
            "test".to_string().into_bytes(),
        );
        assert!(result.is_ok())
    });

    sled_test!(read_without_key, |storage: SledStorageProvider| {
        storage
            .write(
                1,
                "key".to_string().into_bytes(),
                "test".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                1,
                "key2".to_string().into_bytes(),
                "test2".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                2,
                "key3".to_string().into_bytes(),
                "test3".to_string().into_bytes(),
            )
            .unwrap();
        let result = storage.read(1, None).unwrap();

        assert_eq!(2, result.len());
    });

    sled_test!(read_with_key, |storage: SledStorageProvider| {
        storage
            .write(
                1,
                "key".to_string().into_bytes(),
                "test".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                1,
                "key2".to_string().into_bytes(),
                "test2".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                2,
                "key3".to_string().into_bytes(),
                "test3".to_string().into_bytes(),
            )
            .unwrap();
        let result = storage
            .read(1, Some("key2".to_string().into_bytes()))
            .unwrap();

        assert_eq!(1, result.len());
    });

    sled_test!(
        read_with_non_existing_key,
        |storage: SledStorageProvider| {
            let result = storage
                .read(1, Some("non_existing".to_string().into_bytes()))
                .unwrap();
            assert_eq!(0, result.len())
        }
    );

    sled_test!(delete_without_key, |storage: SledStorageProvider| {
        storage
            .write(
                1,
                "key".to_string().into_bytes(),
                "test".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                1,
                "key2".to_string().into_bytes(),
                "test2".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                2,
                "key3".to_string().into_bytes(),
                "test3".to_string().into_bytes(),
            )
            .unwrap();

        let result = storage.read(1, None).unwrap();
        assert_eq!(2, result.len());

        let result = storage.delete(1, None);
        assert!(result.is_ok());

        let result = storage.read(1, None).unwrap();
        assert_eq!(0, result.len());
    });

    sled_test!(delete_with_key, |storage: SledStorageProvider| {
        storage
            .write(
                1,
                "key".to_string().into_bytes(),
                "test".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                1,
                "key2".to_string().into_bytes(),
                "test2".to_string().into_bytes(),
            )
            .unwrap();
        storage
            .write(
                2,
                "key3".to_string().into_bytes(),
                "test3".to_string().into_bytes(),
            )
            .unwrap();

        let result = storage.read(1, None).unwrap();
        assert_eq!(2, result.len());

        let result = storage.delete(1, Some("key2".to_string().into_bytes()));
        assert!(result.is_ok());

        let result = storage.read(1, None).unwrap();
        assert_eq!(1, result.len());
    });
}
