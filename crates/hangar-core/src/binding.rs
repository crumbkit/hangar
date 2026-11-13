//! Bindings: a named pin of a page version with the records that fill it.

use crate::name::{BindingName, PageVersion};
use crate::page::{Fills, PagePin};
use crate::provenance::{Provenance, WriteProvenance};

/// A named choice of a page version and the records that fill its slots.
///
/// The only constructor takes a [`Provenance`], and the type has no
/// `Default`, so a binding without provenance does not compile:
///
/// ```compile_fail,E0061
/// # use hangar_core::*;
/// # let name = BindingName::try_from("my-view")?;
/// # let pin = PagePin::new(PageName::try_from("home")?, PageVersion::try_from("v1")?);
/// # let fills = Fills::new([(
/// #     SlotName::try_from("tasks")?,
/// #     RecordPath::try_from("/tasks/a")?,
/// # )])?;
/// let _ = Binding::new(name, pin, fills);
/// # Ok::<(), ParseError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Binding {
    name: BindingName,
    pin: PagePin,
    fills: Fills,
    provenance: Provenance,
    last_move: Option<WriteProvenance>,
}

impl Binding {
    /// Builds a binding that no move has changed. A binding made straight
    /// from a page states [`MadeFrom::Page`](crate::MadeFrom::Page) with that
    /// page's pin in its provenance.
    #[must_use]
    pub fn new(name: BindingName, pin: PagePin, fills: Fills, provenance: Provenance) -> Self {
        Self {
            name,
            pin,
            fills,
            provenance,
            last_move: None,
        }
    }

    /// The binding's name.
    #[must_use]
    pub fn name(&self) -> &BindingName {
        &self.name
    }

    /// The page version the binding is pinned to.
    #[must_use]
    pub fn pin(&self) -> &PagePin {
        &self.pin
    }

    /// The records that fill the page's slots.
    #[must_use]
    pub fn fills(&self) -> &Fills {
        &self.fills
    }

    /// Who made the binding and from what.
    #[must_use]
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Who moved the pin last, and through which binding. `None` for a
    /// binding that no move has changed. [`Binding::provenance`] keeps saying
    /// who saved the binding.
    #[must_use]
    pub fn last_move(&self) -> Option<&WriteProvenance> {
        self.last_move.as_ref()
    }

    /// The same binding with `mover` as the provenance of its last move. A
    /// store that keeps bindings outside this crate calls it when it reads a
    /// binding back that a move changed.
    #[must_use]
    pub fn with_last_move(self, mover: WriteProvenance) -> Self {
        Self {
            last_move: Some(mover),
            ..self
        }
    }

    /// The same binding pinned to another version of its page, with `mover`
    /// as the provenance of its last move.
    #[must_use]
    pub(crate) fn moved_to(&self, version: PageVersion, mover: WriteProvenance) -> Self {
        Self {
            pin: PagePin::new(self.pin.name().clone(), version),
            last_move: Some(mover),
            ..self.clone()
        }
    }
}
