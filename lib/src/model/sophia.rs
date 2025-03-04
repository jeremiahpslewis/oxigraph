//! This crate provides implementation of [Sophia](https://docs.rs/sophia/) traits for the `model` module.

use crate::model::*;
use sophia_api::term::*;
use std::fmt;

impl TTerm for BlankNode {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for BlankNode {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::BlankNode => Ok(Self::new_unchecked(other.value_raw().0)),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for BlankNodeRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TTerm for Literal {
    fn kind(&self) -> TermKind {
        TermKind::Literal
    }

    fn value_raw(&self) -> RawValue<'_> {
        Self::value(self).into()
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        Some(SimpleIri::new_unchecked(
            Self::datatype(self).as_str(),
            None,
        ))
    }

    fn language(&self) -> Option<&str> {
        Self::language(self)
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for Literal {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Literal => match other.language() {
                Some(tag) => Ok(Self::new_language_tagged_literal_unchecked(
                    other.value_raw().0,
                    tag,
                )),
                None => Ok(Self::new_typed_literal(
                    other.value_raw().0,
                    other.datatype().unwrap(),
                )),
            },
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for LiteralRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::Literal
    }

    fn value_raw(&self) -> RawValue<'_> {
        LiteralRef::value(*self).into()
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        Some(SimpleIri::new_unchecked(
            LiteralRef::datatype(*self).as_str(),
            None,
        ))
    }

    fn language(&self) -> Option<&str> {
        LiteralRef::language(*self)
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TTerm for NamedNode {
    fn kind(&self) -> TermKind {
        TermKind::Iri
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for NamedNode {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(Self::new_unchecked(other.value())),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> From<SimpleIri<'a>> for NamedNode {
    fn from(other: SimpleIri<'a>) -> Self {
        Self::new_unchecked(other.value())
    }
}

impl<'a> TTerm for NamedNodeRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl From<GraphName> for Option<Term> {
    fn from(other: GraphName) -> Self {
        match other {
            GraphName::NamedNode(n) => Some(n.into()),
            GraphName::BlankNode(n) => Some(n.into()),
            GraphName::DefaultGraph => None,
        }
    }
}

impl<'a> From<GraphNameRef<'a>> for Option<TermRef<'a>> {
    fn from(other: GraphNameRef<'a>) -> Self {
        match other {
            GraphNameRef::NamedNode(n) => Some(n.into()),
            GraphNameRef::BlankNode(n) => Some(n.into()),
            GraphNameRef::DefaultGraph => None,
        }
    }
}

impl TTerm for Subject {
    fn kind(&self) -> TermKind {
        match self {
            Self::NamedNode(_) => TermKind::Iri,
            Self::BlankNode(_) => TermKind::BlankNode,
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        match self {
            Self::NamedNode(n) => n.value_raw(),
            Self::BlankNode(n) => n.value_raw(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        match self {
            Self::NamedNode(n) => n.as_dyn(),
            Self::BlankNode(n) => n.as_dyn(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }
}

impl TryCopyTerm for Subject {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(NamedNode::try_copy(other).unwrap().into()),
            TermKind::BlankNode => Ok(BlankNode::try_copy(other).unwrap().into()),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for SubjectRef<'a> {
    fn kind(&self) -> TermKind {
        match self {
            Self::NamedNode(_) => TermKind::Iri,
            Self::BlankNode(_) => TermKind::BlankNode,
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        match self {
            Self::NamedNode(n) => n.value_raw(),
            Self::BlankNode(n) => n.value_raw(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        match self {
            Self::NamedNode(n) => n.as_dyn(),
            Self::BlankNode(n) => n.as_dyn(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }
}

impl TTerm for Term {
    fn kind(&self) -> TermKind {
        match self {
            Self::NamedNode(_) => TermKind::Iri,
            Self::BlankNode(_) => TermKind::BlankNode,
            Self::Literal(_) => TermKind::Literal,
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        match self {
            Self::NamedNode(n) => n.value_raw(),
            Self::BlankNode(n) => n.value_raw(),
            Self::Literal(l) => l.value_raw(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        if let Self::Literal(l) = self {
            TTerm::datatype(l)
        } else {
            None
        }
    }

    fn language(&self) -> Option<&str> {
        if let Self::Literal(l) = self {
            TTerm::language(l)
        } else {
            None
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        match self {
            Self::NamedNode(n) => n.as_dyn(),
            Self::BlankNode(n) => n.as_dyn(),
            Self::Literal(l) => l.as_dyn(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }
}

impl TryCopyTerm for Term {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(NamedNode::try_copy(other).unwrap().into()),
            TermKind::BlankNode => Ok(BlankNode::try_copy(other).unwrap().into()),
            TermKind::Literal => Ok(Literal::try_copy(other).unwrap().into()),
            TermKind::Variable => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for TermRef<'a> {
    fn kind(&self) -> TermKind {
        match self {
            Self::NamedNode(_) => TermKind::Iri,
            Self::BlankNode(_) => TermKind::BlankNode,
            Self::Literal(_) => TermKind::Literal,
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        if let Self::Literal(l) = self {
            TTerm::datatype(l)
        } else {
            None
        }
    }

    fn language(&self) -> Option<&str> {
        if let Self::Literal(l) = self {
            TTerm::language(l)
        } else {
            None
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        match self {
            Self::NamedNode(n) => n.value_raw(),
            Self::BlankNode(n) => n.value_raw(),
            Self::Literal(l) => l.value_raw(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        match self {
            Self::NamedNode(n) => n.as_dyn(),
            Self::BlankNode(n) => n.as_dyn(),
            Self::Literal(l) => l.as_dyn(),
            Self::Triple(_) => panic!("RDF-star is not supported yet by Sophia"),
        }
    }
}

impl From<Quad> for ([Term; 3], Option<Term>) {
    fn from(other: Quad) -> Self {
        (
            [other.subject.into(), other.predicate.into(), other.object],
            other.graph_name.into(),
        )
    }
}

impl<'a> From<QuadRef<'a>> for ([TermRef<'a>; 3], Option<TermRef<'a>>) {
    fn from(other: QuadRef<'a>) -> Self {
        (
            [other.subject.into(), other.predicate.into(), other.object],
            other.graph_name.into(),
        )
    }
}

impl From<Triple> for [Term; 3] {
    fn from(other: Triple) -> Self {
        [other.subject.into(), other.predicate.into(), other.object]
    }
}

impl<'a> From<TripleRef<'a>> for [TermRef<'a>; 3] {
    fn from(other: TripleRef<'a>) -> Self {
        [other.subject.into(), other.predicate.into(), other.object]
    }
}

/// Error raised when trying to copy a [Sophia](sophia)
/// term as an incompatible Oxigraph term
/// (e.g. a literal into `NamedNode`).
#[derive(Clone, Copy, Debug)]
pub struct SophiaToOxigraphConversionError;

impl fmt::Display for SophiaToOxigraphConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for SophiaToOxigraphConversionError {}
