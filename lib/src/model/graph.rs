//! [In-memory implementation](super::Graph) of  [RDF graphs](https://www.w3.org/TR/rdf11-concepts/#dfn-graph).
//!
//! Usage example:
//! ```
//! use oxigraph::model::*;
//!
//! let mut graph = Graph::default();
//!
//! // insertion
//! let ex = NamedNodeRef::new("http://example.com")?;
//! let triple = TripleRef::new(ex, ex, ex);
//! graph.insert(triple);
//!
//! // simple filter
//! let results: Vec<_> = graph.triples_for_subject(ex).collect();
//! assert_eq!(vec![triple], results);
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
//! ```
//!
//! See also [`Dataset`](super::Dataset) if you want to get support of multiple RDF graphs at the same time.

use crate::io::GraphFormat;
use crate::model::dataset::*;
use crate::model::*;
use std::io::{BufRead, Write};
use std::{fmt, io};

/// An in-memory [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph).
///
/// It can accomodate a fairly large number of triples (in the few millions).
/// Beware: it interns the string and does not do any garbage collection yet:
/// if you insert and remove a lot of different terms, memory will grow without any reduction.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
///
/// let mut graph = Graph::default();
///
/// // insertion
/// let ex = NamedNodeRef::new("http://example.com")?;
/// let triple = TripleRef::new(ex, ex, ex);
/// graph.insert(triple);
///
/// // simple filter
/// let results: Vec<_> = graph.triples_for_subject(ex).collect();
/// assert_eq!(vec![triple], results);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug, Default)]
pub struct Graph {
    dataset: Dataset,
}

impl Graph {
    /// Creates a new graph
    pub fn new() -> Self {
        Self::default()
    }

    fn graph(&self) -> GraphView<'_> {
        self.dataset.graph(GraphNameRef::DefaultGraph)
    }

    fn graph_mut(&mut self) -> GraphViewMut<'_> {
        self.dataset.graph_mut(GraphNameRef::DefaultGraph)
    }

    /// Returns all the triples contained by the graph
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.graph().iter(),
        }
    }

    pub fn triples_for_subject<'a, 'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.graph()
            .triples_for_interned_subject(self.dataset.encoded_subject(subject))
    }

    pub fn objects_for_subject_predicate<'a, 'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.graph().objects_for_interned_subject_predicate(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub fn object_for_subject_predicate<'a, 'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.graph()
            .objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<'a, 'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.graph().predicates_for_interned_subject_object(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_term(object),
        )
    }

    pub fn triples_for_predicate<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.graph()
            .triples_for_interned_predicate(self.dataset.encoded_named_node(predicate))
    }

    pub fn subjects_for_predicate_object<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = SubjectRef<'a>> + 'a {
        self.graph().subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub fn subject_for_predicate_object<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<SubjectRef<'a>> {
        self.graph().subject_for_predicate_object(predicate, object)
    }

    pub fn triples_for_object<'a, 'b>(
        &'a self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.graph()
            .triples_for_interned_object(self.dataset.encoded_term(object))
    }

    /// Checks if the graph contains the given triple
    pub fn contains<'a>(&self, triple: impl Into<TripleRef<'a>>) -> bool {
        self.graph().contains(triple)
    }

    /// Returns the number of triples in this graph
    pub fn len(&self) -> usize {
        self.dataset.len()
    }

    /// Checks if this graph contains a triple
    pub fn is_empty(&self) -> bool {
        self.dataset.is_empty()
    }

    /// Adds a triple to the graph
    pub fn insert<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> bool {
        self.graph_mut().insert(triple)
    }

    /// Removes a concrete triple from the graph
    pub fn remove<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> bool {
        self.graph_mut().remove(triple)
    }

    /// Clears the graph
    pub fn clear(&mut self) {
        self.dataset.clear()
    }

    /// Loads a file into the graph.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::io::GraphFormat;
    ///
    /// let mut graph = Graph::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// graph.load(file.as_ref(), GraphFormat::NTriples, None)?;
    ///
    /// // we inspect the graph contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(graph.contains(TripleRef::new(ex, ex, ex)));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning: This functions inserts the triples during the parsing.
    /// If the parsing fails in the middle of the file, the triples read before stay in the graph.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load(
        &mut self,
        reader: impl BufRead,
        format: GraphFormat,
        base_iri: Option<&str>,
    ) -> io::Result<()> {
        self.graph_mut().load(reader, format, base_iri)
    }

    /// Dumps the graph into a file.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::Graph;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let mut graph = Graph::new();
    /// graph.load(file, GraphFormat::NTriples, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// graph.dump(&mut buffer, GraphFormat::NTriples)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn dump(&self, writer: impl Write, format: GraphFormat) -> io::Result<()> {
        self.graph().dump(writer, format)
    }

    /// Applies on the graph the canonicalization process described in
    /// [Canonical Forms for Isomorphic and Equivalent RDF Graphs: Algorithms for Leaning and Labelling Blank Nodes, Aidan Hogan, 2017](http://aidanhogan.com/docs/rdf-canonicalisation.pdf)
    ///   
    /// Usage example ([Graph isomorphim](https://www.w3.org/TR/rdf11-concepts/#dfn-graph-isomorphism)):
    /// ```
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::Graph;
    ///
    /// let file = "<http://example.com> <http://example.com> [ <http://example.com/p> <http://example.com/o> ] .".as_bytes();
    ///
    /// let mut graph1 = Graph::new();
    /// graph1.load(file, GraphFormat::Turtle, None)?;
    /// let mut graph2 = Graph::new();
    /// graph2.load(file, GraphFormat::Turtle, None)?;
    ///
    /// assert_ne!(graph1, graph2);
    /// graph1.canonicalize();
    /// graph2.canonicalize();
    /// assert_eq!(graph1, graph2);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning 1: Blank node ids depends on the current shape of the graph. Adding a new triple might change the ids of a lot of blank nodes.
    /// Hence, this canonization might not be suitable for diffs.
    ///
    /// Warning 2: The canonicalization algorithm is not stable and canonical blank node Ids might change between Oxigraph version.
    ///
    /// Warning 3: This implementation worst-case complexity is in *O(b!)* with b the number of blank nodes in the input graph.
    pub fn canonicalize(&mut self) {
        self.dataset.canonicalize()
    }
}

impl PartialEq for Graph {
    fn eq(&self, other: &Self) -> bool {
        self.dataset == other.dataset
    }
}

impl Eq for Graph {}

impl<'a> IntoIterator for &'a Graph {
    type Item = TripleRef<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl FromIterator<Triple> for Graph {
    fn from_iter<I: IntoIterator<Item = Triple>>(iter: I) -> Self {
        let mut g = Self::new();
        g.extend(iter);
        g
    }
}

impl<'a, T: Into<TripleRef<'a>>> FromIterator<T> for Graph {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut g = Self::new();
        g.extend(iter);
        g
    }
}

impl Extend<Triple> for Graph {
    fn extend<I: IntoIterator<Item = Triple>>(&mut self, iter: I) {
        self.graph_mut().extend(iter)
    }
}

impl<'a, T: Into<TripleRef<'a>>> Extend<T> for Graph {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.graph_mut().extend(iter)
    }
}

impl fmt::Display for Graph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.graph().fmt(f)
    }
}

/// Iterator returned by [`Graph::iter`]
pub struct Iter<'a> {
    inner: GraphViewIter<'a>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = TripleRef<'a>;

    fn next(&mut self) -> Option<TripleRef<'a>> {
        self.inner.next()
    }
}
