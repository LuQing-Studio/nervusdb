//! Minimal Write-Ahead Log placeholder.

use std::path::PathBuf;

use crate::error::Result;
use crate::triple::Triple;

#[derive(Debug, Clone, Copy)]
pub enum WalRecordType {
    AddTriple,
}

#[derive(Debug)]
pub struct WalEntry {
    pub record_type: WalRecordType,
    pub triple: Triple,
}

#[derive(Debug)]
pub struct WriteAheadLog {
    path: PathBuf,
    buffer: Vec<WalEntry>,
}

impl WriteAheadLog {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            buffer: Vec::new(),
        }
    }

    pub fn append(&mut self, entry: WalEntry) {
        self.buffer.push(entry);
    }

    pub fn flush(&mut self) -> Result<()> {
        // TODO: write to disk. For now we just clear the buffer to keep semantics simple.
        self.buffer.clear();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
