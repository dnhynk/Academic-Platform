//! Which aggregation key one entity's evidence belongs to, at the granularity
//! the user selected.
//!
//! The tree is `P2-N1`'s. A [`FieldResolver`] is built from one
//! `VersionedTaxonomyImport`, so the key an item aggregates under is a fact
//! about the exact taxonomy release the scope names — a concept moved to
//! another field in a later release resolves differently there, and the digest
//! that binds the release is what makes the two answers distinguishable.
//!
//! ## An entity the release does not hold resolves to nothing
//!
//! [`FieldResolver::resolve`] answers `None`, and
//! [`crate::coverage::FieldCoverage::of`] refuses an item that resolved to
//! another key or to none at all. `P2-N2` found this defect one layer up — an
//! `APPLIED` state for one concept projected out of another concept's admitted
//! evidence — and `P2-N3` found the one-hop form of it. Coverage is the third
//! place the same mistake fits: a count is exactly the kind of value that
//! silently absorbs an item about something else.

use std::collections::BTreeMap;

use academic_domain::{
    EntityId,
    ontology::{TaxonomyNode, VersionedTaxonomyImport},
};

use crate::scope::TaxonomyGranularity;

/// One release's entity-to-aggregation-key map at one granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldResolver {
    granularity: TaxonomyGranularity,
    keys: BTreeMap<EntityId, EntityId>,
}

impl FieldResolver {
    /// Reads `import`'s tree at `granularity`.
    ///
    /// At `FIELD` a concept resolves to its field and an operation to its
    /// concept's field; at `CONCEPT` an operation resolves to its concept and a
    /// concept to itself; at `OPERATION` only an operation resolves, to itself.
    /// A node above the selected tier has no key, because aggregating a field's
    /// own evidence under one of its concepts would attribute it to a concept
    /// nothing was recorded about.
    #[must_use]
    pub fn of(import: &VersionedTaxonomyImport, granularity: TaxonomyGranularity) -> Self {
        let mut concept_field: BTreeMap<EntityId, EntityId> = BTreeMap::new();
        let mut operation_concept: BTreeMap<EntityId, EntityId> = BTreeMap::new();
        for node in import.nodes() {
            match node {
                TaxonomyNode::Field(_) => {}
                TaxonomyNode::Concept(concept) => {
                    concept_field.insert(concept.id(), concept.field_id());
                }
                TaxonomyNode::Operation(operation) => {
                    operation_concept.insert(operation.id(), operation.concept_id());
                }
            }
        }

        let mut keys: BTreeMap<EntityId, EntityId> = BTreeMap::new();
        for node in import.nodes() {
            let key = match (granularity, node) {
                (TaxonomyGranularity::Field, TaxonomyNode::Field(field)) => Some(field.id()),
                (TaxonomyGranularity::Field, TaxonomyNode::Concept(concept)) => {
                    Some(concept.field_id())
                }
                (TaxonomyGranularity::Field, TaxonomyNode::Operation(operation)) => {
                    concept_field.get(&operation.concept_id()).copied()
                }
                (TaxonomyGranularity::Concept, TaxonomyNode::Concept(concept)) => {
                    Some(concept.id())
                }
                (TaxonomyGranularity::Concept, TaxonomyNode::Operation(operation)) => {
                    Some(operation.concept_id())
                }
                (TaxonomyGranularity::Operation, TaxonomyNode::Operation(operation)) => {
                    Some(operation.id())
                }
                (
                    TaxonomyGranularity::Concept | TaxonomyGranularity::Operation,
                    TaxonomyNode::Field(_),
                )
                | (TaxonomyGranularity::Operation, TaxonomyNode::Concept(_)) => None,
            };
            if let Some(key) = key {
                keys.insert(node.id(), key);
            }
        }
        Self { granularity, keys }
    }

    /// The granularity this resolver was built at.
    #[must_use]
    pub const fn granularity(&self) -> TaxonomyGranularity {
        self.granularity
    }

    /// Which key `entity`'s evidence aggregates under, if any.
    #[must_use]
    pub fn resolve(&self, entity: EntityId) -> Option<EntityId> {
        self.keys.get(&entity).copied()
    }

    /// Every distinct aggregation key the release holds, in identity order.
    ///
    /// This is the population a skew explanation is read over: a key with no
    /// evidence at all is a key that appears here and in no coverage reading.
    #[must_use]
    pub fn keys(&self) -> Vec<EntityId> {
        let mut found: Vec<EntityId> = self.keys.values().copied().collect();
        found.sort_unstable();
        found.dedup();
        found
    }
}
