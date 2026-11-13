//! Deliberately wrong stores, for the tests that show each contract suite
//! catches the flaw it exists to catch. Each flaw breaks one clause and
//! keeps every other rule, so a catch test shows that the suite checks that
//! clause.

use crate::fixed_authenticator::FixedAuthenticator;
use hangar_core::memory::{MemoryPageStore, MemoryRecordStore};
use hangar_core::{
    Access, AuthError, Authenticator, Binding, BindingName, Blob, Credential, Decision, MadeFrom,
    Page, PageName, PagePin, PageStore, PageStoreError, PageVersion, Principal, Provenance,
    PublishedPage, ReadError, Record, RecordBody, RecordPath, RecordStore, RecordType, Resource,
    WriteError, WriteProvenance,
};
use std::collections::HashMap;
use std::sync::Arc;

/// An authenticator that answers like the one it wraps, except that it
/// allows every principal to reach every resource.
pub(crate) struct AllowsEverything(pub(crate) FixedAuthenticator);

impl Authenticator for AllowsEverything {
    async fn authenticate(&self, credential: &Credential) -> Result<Principal, AuthError> {
        self.0.authenticate(credential).await
    }

    async fn authorize(
        &self,
        _principal: &Principal,
        _access: Access,
        _resource: &Resource,
    ) -> Result<Decision, AuthError> {
        Ok(Decision::Allowed)
    }
}

/// A record store that writes without comparing the blob the caller names.
pub(crate) struct IgnoresExpectedBlob(pub(crate) MemoryRecordStore);

impl RecordStore for IgnoresExpectedBlob {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        self.0.read(path).await
    }

    async fn write(
        &self,
        path: &RecordPath,
        body: &RecordBody,
        provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        let current = self.0.read(path).await.map_err(|_| WriteError::NotFound)?;
        let forced = WriteProvenance::new(
            current.blob().clone(),
            provenance.made_by().clone(),
            provenance.through().clone(),
        );
        self.0.write(path, body, &forced).await
    }
}

/// The flaw that a [`FlawedPageStore`] has.
#[derive(Clone, Copy)]
pub(crate) enum Flaw {
    /// A page version is stored with different provenance from the one given.
    DropsProvenance,
    /// `publish` and `save_binding` accept a `made_from` that names nothing.
    AcceptsUnresolvedOrigin,
    /// `save_binding` accepts a `made_from` that names an unknown page
    /// version. `publish` and the other origins are checked.
    AcceptsUnresolvedPageOriginOnSave,
    /// A republish moves every binding of the page to the new version.
    MovesPinsOnRepublish,
    /// `save_binding` does not compare the type of a filled record with the
    /// type of its slot. It checks everything else.
    SkipsTypeCheckOnSave,
    /// `move_binding` does not compare the type of a filled record with the
    /// type of its slot. It checks everything else.
    SkipsTypeCheckOnMove,
    /// `move_binding` does not read the bound records, so a record that has
    /// vanished is not noticed.
    SkipsExistenceCheckOnMove,
}

/// A page store that is correct except for one flaw.
pub(crate) struct FlawedPageStore {
    inner: MemoryPageStore,
    records: Arc<MemoryRecordStore>,
    flaw: Flaw,
}

impl FlawedPageStore {
    pub(crate) fn new(flaw: Flaw, records: Arc<MemoryRecordStore>) -> Self {
        Self {
            inner: MemoryPageStore::new(),
            records,
            flaw,
        }
    }
}

/// A record store that reads the records of the store it wraps, except that
/// it reports the paths it was given at the type its slot declares. The fit
/// check then sees no wrong type, and still sees a missing record.
struct TypeBlind<'a, R> {
    inner: &'a R,
    retyped: HashMap<RecordPath, RecordType>,
}

impl<'a, R> TypeBlind<'a, R> {
    fn for_binding(page: &Page, binding: &Binding, inner: &'a R) -> Self {
        let mut retyped = HashMap::new();
        for (slot, path) in binding.fills().iter() {
            if let Some(declared) = page.slots().get(slot) {
                retyped.insert(path.clone(), declared.record_type().clone());
            }
        }
        Self { inner, retyped }
    }
}

impl<R: RecordStore> RecordStore for TypeBlind<'_, R> {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        let record = self.inner.read(path).await?;
        match self.retyped.get(path) {
            Some(declared) => Ok(Record::new(
                path.clone(),
                declared.clone(),
                record.blob().clone(),
                record.body().clone(),
            )),
            None => Ok(record),
        }
    }

    async fn write(
        &self,
        _path: &RecordPath,
        _body: &RecordBody,
        _provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        Err(WriteError::Unavailable)
    }
}

/// A record store that invents a record of the declared type for every
/// filled path of a declared slot, so the fit check finds every record.
struct Invented(HashMap<RecordPath, RecordType>);

impl Invented {
    fn for_binding(page: &Page, binding: &Binding) -> Self {
        let mut types = HashMap::new();
        for (slot, path) in binding.fills().iter() {
            if let Some(declared) = page.slots().get(slot) {
                types.insert(path.clone(), declared.record_type().clone());
            }
        }
        Self(types)
    }
}

impl RecordStore for Invented {
    async fn read(&self, path: &RecordPath) -> Result<Record, ReadError> {
        let record_type = self.0.get(path).ok_or(ReadError::NotFound)?;
        Ok(Record::new(
            path.clone(),
            record_type.clone(),
            Blob::try_from("a".repeat(40).as_str()).map_err(|_| ReadError::Unavailable)?,
            RecordBody::try_from("").map_err(|_| ReadError::Unavailable)?,
        ))
    }

    async fn write(
        &self,
        _path: &RecordPath,
        _body: &RecordBody,
        _provenance: &WriteProvenance,
    ) -> Result<Blob, WriteError> {
        Err(WriteError::Unavailable)
    }
}

/// The binding with its provenance replaced by `Original`.
fn made_from_nothing(binding: &Binding) -> Result<Binding, PageStoreError> {
    let provenance = Provenance::new(
        binding.provenance().made_by().clone(),
        MadeFrom::Original,
        Vec::new(),
    )
    .map_err(|_| PageStoreError::Unavailable)?;
    Ok(Binding::new(
        binding.name().clone(),
        binding.pin().clone(),
        binding.fills().clone(),
        provenance,
    ))
}

impl PageStore for FlawedPageStore {
    async fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> Result<PageVersion, PageStoreError> {
        let name = page.name().clone();
        let provenance = match self.flaw {
            Flaw::DropsProvenance | Flaw::AcceptsUnresolvedOrigin => {
                Provenance::new(provenance.made_by().clone(), MadeFrom::Original, Vec::new())
                    .map_err(|_| PageStoreError::Unavailable)?
            }
            _ => provenance,
        };
        let version = self.inner.publish(page, provenance).await?;
        if matches!(self.flaw, Flaw::MovesPinsOnRepublish) {
            for binding in self.inner.bindings_of(&name).await? {
                self.inner
                    .move_binding(binding.name(), &version, &*self.records)
                    .await?;
            }
        }
        Ok(version)
    }

    async fn latest(&self, name: &PageName) -> Result<PublishedPage, PageStoreError> {
        self.inner.latest(name).await
    }

    async fn at(&self, pin: &PagePin) -> Result<PublishedPage, PageStoreError> {
        self.inner.at(pin).await
    }

    async fn versions(&self, name: &PageName) -> Result<Vec<PageVersion>, PageStoreError> {
        self.inner.versions(name).await
    }

    async fn pages(&self) -> Result<Vec<PageName>, PageStoreError> {
        self.inner.pages().await
    }

    async fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        match self.flaw {
            Flaw::SkipsTypeCheckOnSave => {
                let page = self.inner.at(binding.pin()).await?;
                let blind = TypeBlind::for_binding(page.page(), &binding, records);
                self.inner.save_binding(binding, &blind).await
            }
            Flaw::AcceptsUnresolvedOrigin => {
                let rewritten = made_from_nothing(&binding)?;
                self.inner.save_binding(rewritten, records).await
            }
            Flaw::AcceptsUnresolvedPageOriginOnSave
                if matches!(binding.provenance().made_from(), MadeFrom::Page(_)) =>
            {
                let rewritten = made_from_nothing(&binding)?;
                self.inner.save_binding(rewritten, records).await
            }
            _ => self.inner.save_binding(binding, records).await,
        }
    }

    async fn binding(&self, name: &BindingName) -> Result<Binding, PageStoreError> {
        self.inner.binding(name).await
    }

    async fn bindings_of(&self, name: &PageName) -> Result<Vec<Binding>, PageStoreError> {
        self.inner.bindings_of(name).await
    }

    async fn move_binding(
        &self,
        name: &BindingName,
        version: &PageVersion,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        match self.flaw {
            Flaw::SkipsTypeCheckOnMove | Flaw::SkipsExistenceCheckOnMove => {
                let binding = self.inner.binding(name).await?;
                let target = PagePin::new(binding.pin().name().clone(), version.clone());
                let page = self.inner.at(&target).await?;
                if matches!(self.flaw, Flaw::SkipsTypeCheckOnMove) {
                    let blind = TypeBlind::for_binding(page.page(), &binding, records);
                    self.inner.move_binding(name, version, &blind).await
                } else {
                    let invented = Invented::for_binding(page.page(), &binding);
                    self.inner.move_binding(name, version, &invented).await
                }
            }
            _ => self.inner.move_binding(name, version, records).await,
        }
    }
}

/// A page store whose version names are not `v1`, `v2` and so on, as in a
/// store whose versions are commit ids. It prefixes the memory store's
/// version names with `c0ffee`. Only `publish`, `save_binding` and
/// `bindings_of` translate names, which is all the tests that use it call.
pub(crate) struct PrefixedVersions(pub(crate) MemoryPageStore);

const PREFIX: &str = "c0ffee";

fn prefixed(version: &PageVersion) -> Result<PageVersion, PageStoreError> {
    PageVersion::try_from(format!("{PREFIX}{}", version.as_str()).as_str())
        .map_err(|_| PageStoreError::Unavailable)
}

fn unprefixed(version: &PageVersion) -> Result<PageVersion, PageStoreError> {
    let bare = version
        .as_str()
        .strip_prefix(PREFIX)
        .ok_or(PageStoreError::UnknownVersion)?;
    PageVersion::try_from(bare).map_err(|_| PageStoreError::Unavailable)
}

fn repinned(
    binding: &Binding,
    translate: fn(&PageVersion) -> Result<PageVersion, PageStoreError>,
) -> Result<Binding, PageStoreError> {
    let pin = PagePin::new(
        binding.pin().name().clone(),
        translate(binding.pin().version())?,
    );
    Ok(Binding::new(
        binding.name().clone(),
        pin,
        binding.fills().clone(),
        binding.provenance().clone(),
    ))
}

impl PageStore for PrefixedVersions {
    async fn publish(
        &self,
        page: Page,
        provenance: Provenance,
    ) -> Result<PageVersion, PageStoreError> {
        prefixed(&self.0.publish(page, provenance).await?)
    }

    async fn latest(&self, name: &PageName) -> Result<PublishedPage, PageStoreError> {
        self.0.latest(name).await
    }

    async fn at(&self, pin: &PagePin) -> Result<PublishedPage, PageStoreError> {
        self.0.at(pin).await
    }

    async fn versions(&self, name: &PageName) -> Result<Vec<PageVersion>, PageStoreError> {
        self.0.versions(name).await
    }

    async fn pages(&self) -> Result<Vec<PageName>, PageStoreError> {
        self.0.pages().await
    }

    async fn save_binding(
        &self,
        binding: Binding,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        self.0
            .save_binding(repinned(&binding, unprefixed)?, records)
            .await
    }

    async fn binding(&self, name: &BindingName) -> Result<Binding, PageStoreError> {
        self.0.binding(name).await
    }

    async fn bindings_of(&self, name: &PageName) -> Result<Vec<Binding>, PageStoreError> {
        let mut found = Vec::new();
        for binding in self.0.bindings_of(name).await? {
            found.push(repinned(&binding, prefixed)?);
        }
        Ok(found)
    }

    async fn move_binding(
        &self,
        name: &BindingName,
        version: &PageVersion,
        records: &impl RecordStore,
    ) -> Result<(), PageStoreError> {
        self.0.move_binding(name, version, records).await
    }
}
