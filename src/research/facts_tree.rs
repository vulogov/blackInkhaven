//! RESRCH-1 (R-P4) — the Facts tree pane. Its generic fold/navigate/reveal
//! machinery was promoted to the shared [`crate::system_tree`] module (so the
//! 1.7 Linguistic companion can reuse it over the Languages book); the Research
//! pane just aliases it and passes `SYSTEM_TAG_FACTS` as the root tag.

pub(super) use crate::system_tree::{SystemBookRow as FactsRow, SystemBookTree as FactsTree};
