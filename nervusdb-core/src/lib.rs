//! NervusDB core Rust library providing the low level storage primitives.

mod dictionary;
mod error;
mod global_index;
mod partition;
mod triple;
mod wal;

use std::path::{Path, PathBuf};

pub use dictionary::{Dictionary, StringId};
pub use error::{Error, Result};
pub use global_index::GlobalIndex;
pub use partition::{PartitionConfig, PartitionId};
pub use triple::{Fact, Triple};
pub use wal::{WalEntry, WalRecordType, WriteAheadLog};

/// Database configuration used when opening an instance.
#[derive(Debug, Clone)]
pub struct Options {
    data_path: PathBuf,
    partition: PartitionConfig,
}

impl Options {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            data_path: path.as_ref().to_owned(),
            partition: PartitionConfig::default(),
        }
    }

    pub fn with_partition_config(mut self, config: PartitionConfig) -> Self {
        self.partition = config;
        self
    }
}

#[derive(Debug)]
pub struct Database {
    options: Options,
    dictionary: Dictionary,
    index: GlobalIndex,
    wal: WriteAheadLog,
    triples: Vec<Triple>,
}

impl Database {
    pub fn open(options: Options) -> Result<Self> {
        let wal_path = options.data_path.join("wal.log");
        Ok(Self {
            wal: WriteAheadLog::new(wal_path),
            options,
            dictionary: Dictionary::new(),
            index: GlobalIndex::new(),
            triples: Vec::new(),
        })
    }

    pub fn add_fact(&mut self, fact: Fact<'_>) -> Result<Triple> {
        let subject = self.dictionary.get_or_insert(fact.subject);
        let predicate = self.dictionary.get_or_insert(fact.predicate);
        let object = self.dictionary.get_or_insert(fact.object);
        let triple = Triple::new(subject, predicate, object);

        let partition = self
            .options
            .partition
            .partition_for(subject, predicate, object);
        self.index.insert(triple, partition);
        self.wal.append(WalEntry {
            record_type: WalRecordType::AddTriple,
            triple,
        });
        self.triples.push(triple);
        Ok(triple)
    }

    pub fn all_triples(&self) -> &[Triple] {
        &self.triples
    }

    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_insert() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Database::open(Options::new(tmp.path())).unwrap();
        let triple = db.add_fact(Fact::new("alice", "knows", "bob")).unwrap();
        assert_eq!(db.all_triples(), &[triple]);
        assert_eq!(
            db.dictionary().lookup_value(triple.subject_id).unwrap(),
            "alice"
        );
    }
}
