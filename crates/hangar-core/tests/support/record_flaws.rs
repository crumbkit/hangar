//! Record stores that break the write contract in one way each, for the
//! tests that show the record suite catches them.

use hangar_core::memory::MemoryRecordStore;
use hangar_core::{
    Blob, ReadError, Record, RecordBody, RecordPath, RecordStore, RecordType, Rejection,
    WriteError, WriteProvenance,
};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

/// A record store that rejects every write, including a write that names an
/// unknown path or a stale blob. It leaves the record unchanged.
pub(crate) struct RejectsEveryWrite(pub(crate) MemoryRecordStore);

impl RecordStore for RejectsEveryWrite {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        self.0.read(path).await
    }

    async fn write(
        &self,
        _path: &RecordPath,
        _body: &RecordBody,
        _provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        Err(WriteError::Rejected(Rejection::new("refused")))
    }
}

/// A record store that writes the body and then answers `Rejected`, so the
/// record changes although the caller is told it did not.
pub(crate) struct RejectsAndWrites(pub(crate) MemoryRecordStore);

impl RecordStore for RejectsAndWrites {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        self.0.read(path).await
    }

    async fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        self.0.write(path, body, provenance).await?;
        Err(WriteError::Rejected(Rejection::new("refused")))
    }
}

/// A record store whose blob is a hash of the body, so a record that goes
/// from one body to another and back has its first blob again.
#[derive(Default)]
pub(crate) struct ContentAddressed {
    records: Mutex<HashMap<RecordPath, (RecordType, RecordBody)>>,
}

fn blob_of(body: &RecordBody) -> Result<Blob, WriteError> {
    let mut hasher = DefaultHasher::new();
    body.as_str().hash(&mut hasher);
    Blob::try_from(format!("{:040x}", hasher.finish()).as_str())
        .map_err(|_| WriteError::Unavailable)
}

impl ContentAddressed {
    pub(crate) fn seed(
        &self,
        path: RecordPath,
        record_type: RecordType,
        body: RecordBody,
    ) -> Result<(), WriteError> {
        let mut records = self.records.lock().map_err(|_| WriteError::Unavailable)?;
        records.insert(path, (record_type, body));
        Ok(())
    }
}

impl RecordStore for ContentAddressed {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        let records = self.records.lock().map_err(|_| ReadError::Unavailable)?;
        let (record_type, body) = records.get(path).ok_or(ReadError::NotFound)?;
        let blob = blob_of(body).map_err(|_| ReadError::Unavailable)?;
        Ok(Record::new(
            path.clone(),
            record_type.clone(),
            blob,
            body.clone(),
        ))
    }

    async fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        let mut records = self.records.lock().map_err(|_| WriteError::Unavailable)?;
        let (_, current) = records.get_mut(path).ok_or(WriteError::NotFound)?;
        if blob_of(current)? != *provenance.based_on() {
            return Err(WriteError::Conflict);
        }
        *current = body.clone();
        blob_of(body)
    }
}
