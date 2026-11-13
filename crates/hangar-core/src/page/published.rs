//! A page version and the pin that names it.

use super::Page;
use crate::name::{PageName, PageVersion};
use crate::provenance::Provenance;

/// A page name with one of its versions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct PagePin {
    name: PageName,
    version: PageVersion,
}

impl PagePin {
    /// Builds a pin.
    #[must_use]
    pub fn new(name: PageName, version: PageVersion) -> Self {
        Self { name, version }
    }

    /// The pinned page.
    #[must_use]
    pub fn name(&self) -> &PageName {
        &self.name
    }

    /// The pinned version.
    #[must_use]
    pub fn version(&self) -> &PageVersion {
        &self.version
    }
}

/// One version of a page together with who made it and from what.
///
/// The only constructor takes a [`Provenance`], and the type has no
/// `Default`, so a published page without provenance does not compile:
///
/// ```compile_fail,E0061
/// # use hangar_core::*;
/// # let page = Page::new(
/// #     PageName::try_from("home")?,
/// #     PageContent::new(ContentFormat::Html, "<p>hi</p>")?,
/// #     Slots::new(Vec::new())?,
/// # );
/// # let version = PageVersion::try_from("v1")?;
/// let _ = PublishedPage::new(page, version);
/// # Ok::<(), ParseError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PublishedPage {
    page: Page,
    version: PageVersion,
    provenance: Provenance,
}

impl PublishedPage {
    /// Builds a published page.
    #[must_use]
    pub fn new(page: Page, version: PageVersion, provenance: Provenance) -> Self {
        Self {
            page,
            version,
            provenance,
        }
    }

    /// The page.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// The version this is.
    #[must_use]
    pub fn version(&self) -> &PageVersion {
        &self.version
    }

    /// Who made this version and from what.
    #[must_use]
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}
