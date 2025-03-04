#![allow(clippy::needless_option_as_deref)]

use crate::io::{map_io_err, PyFileLike};
use crate::model::*;
use crate::sparql::*;
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::GraphNameRef;
use oxigraph::sparql::Update;
use oxigraph::store::{self, Store};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::{Py, PyRef};
use std::io::BufReader;

/// Disk-based RDF store.
///
/// It encodes a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and allows to query it using SPARQL.
/// It is based on the `RocksDB <https://rocksdb.org/>`_ key-value database.
///
/// This store ensure the "repeatable read" isolation level: the store only exposes changes that have
/// been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration
/// of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
///
/// :param path: the path of the directory in which the store should read and write its data. If the directory does not exist, it is created. If no directory is provided a temporary one is created and removed when the Python garbage collector removes the store.
/// :type path: str or None, optional.
/// :raises IOError: if the target directory contains invalid data or could not be accessed.
///
/// The :py:func:`str` function provides a serialization of the store in NQuads:
///
/// >>> store = Store()
/// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
/// >>> str(store)
/// '<http://example.com> <http://example.com/p> "1" <http://example.com/g> .\n'
#[pyclass(name = "Store", module = "oxigraph")]
#[pyo3(text_signature = "(path = None)")]
#[derive(Clone)]
pub struct PyStore {
    inner: Store,
}

#[pymethods]
impl PyStore {
    #[new]
    fn new(path: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            inner: if let Some(path) = path {
                Store::open(path)
            } else {
                Store::new()
            }
            .map_err(map_io_err)?,
        })
    }

    /// Adds a quad to the store.
    ///
    /// :param quad: the quad to add.
    /// :type quad: Quad
    /// :raises IOError: if an I/O error happens during the quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(text_signature = "($self, quad)")]
    fn add(&self, quad: &PyQuad) -> PyResult<()> {
        self.inner.insert(quad).map_err(map_io_err)?;
        Ok(())
    }

    /// Removes a quad from the store.
    ///
    /// :param quad: the quad to remove.
    /// :type quad: Quad
    /// :raises IOError: if an I/O error happens during the quad removal.
    ///
    /// >>> store = Store()
    /// >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    /// >>> store.add(quad)
    /// >>> store.remove(quad)
    /// >>> list(store)
    /// []
    #[pyo3(text_signature = "($self, quad)")]
    fn remove(&self, quad: &PyQuad) -> PyResult<()> {
        self.inner.remove(quad).map_err(map_io_err)?;
        Ok(())
    }

    /// Looks for the quads matching a given pattern.
    ///
    /// :param subject: the quad subject or :py:const:`None` to match everything.
    /// :type subject: NamedNode or BlankNode or None
    /// :param predicate: the quad predicate or :py:const:`None` to match everything.
    /// :type predicate: NamedNode or None
    /// :param object: the quad object or :py:const:`None` to match everything.
    /// :type object: NamedNode or BlankNode or Literal or None
    /// :param graph: the quad graph name. To match only the default graph, use :py:class:`DefaultGraph`. To match everything use :py:const:`None`.
    /// :type graph: NamedNode or BlankNode or DefaultGraph or None
    /// :return: an iterator of the quads matching the pattern.
    /// :rtype: iter(Quad)
    /// :raises IOError: if an I/O error happens during the quads lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.quads_for_pattern(NamedNode('http://example.com'), None, None, None))
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(text_signature = "($self, subject, predicate, object, graph_name = None)")]
    fn quads_for_pattern(
        &self,
        subject: &PyAny,
        predicate: &PyAny,
        object: &PyAny,
        graph_name: Option<&PyAny>,
    ) -> PyResult<QuadIter> {
        let (subject, predicate, object, graph_name) =
            extract_quads_pattern(subject, predicate, object, graph_name)?;
        Ok(QuadIter {
            inner: self.inner.quads_for_pattern(
                subject.as_ref().map(|p| p.into()),
                predicate.as_ref().map(|p| p.into()),
                object.as_ref().map(|p| p.into()),
                graph_name.as_ref().map(|p| p.into()),
            ),
        })
    }

    /// Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.
    ///
    /// :param query: the query to execute.
    /// :type query: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL query or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param use_default_graph_as_union: if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
    /// :type use_default_graph_as_union: bool, optional
    /// :param default_graph: list of the graphs that should be used as the query default graph. By default, the store default graph is used.
    /// :type default_graph: NamedNode or BlankNode or DefaultGraph or list(NamedNode or BlankNode or DefaultGraph) or None, optional
    /// :param named_graphs: list of the named graphs that could be used in SPARQL `GRAPH` clause. By default, all the store named graphs are available.
    /// :type named_graphs: list(NamedNode or BlankNode) or None, optional
    /// :return: a :py:class:`bool` for ``ASK`` queries, an iterator of :py:class:`Triple` for ``CONSTRUCT`` and ``DESCRIBE`` queries and an iterator of :py:class:`QuerySolution` for ``SELECT`` queries.
    /// :rtype: QuerySolutions or QueryTriples or bool
    /// :raises SyntaxError: if the provided query is invalid.
    /// :raises IOError: if an I/O error happens while reading the store.
    ///
    /// ``SELECT`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> list(solution['s'] for solution in store.query('SELECT ?s WHERE { ?s ?p ?o }'))
    /// [<NamedNode value=http://example.com>]
    ///
    /// ``CONSTRUCT`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
    /// [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
    ///
    /// ``ASK`` query:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.query('ASK { ?s ?p ?o }')
    /// True
    #[pyo3(
        text_signature = "($self, query, *, base_iri, use_default_graph_as_union, default_graph, named_graphs)"
    )]
    #[args(
        query,
        "*",
        base_iri = "None",
        use_default_graph_as_union = "false",
        default_graph = "None",
        named_graphs = "None"
    )]
    fn query(
        &self,
        query: &str,
        base_iri: Option<&str>,
        use_default_graph_as_union: bool,
        default_graph: Option<&PyAny>,
        named_graphs: Option<&PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyObject> {
        let query = parse_query(
            query,
            base_iri,
            use_default_graph_as_union,
            default_graph,
            named_graphs,
        )?;
        let results = self.inner.query(query).map_err(map_evaluation_error)?;
        query_results_to_python(py, results)
    }

    /// Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.
    ///
    /// Updates are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.
    ///
    /// :param update: the update to execute.
    /// :type update: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL update or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :raises SyntaxError: if the provided update is invalid.
    /// :raises IOError: if an I/O error happens while reading the store.
    ///
    /// The store does not track the existence of empty named graphs.
    /// This method has no ACID guarantees.
    ///
    /// ``INSERT DATA`` update:
    ///
    /// >>> store = Store()
    /// >>> store.update('INSERT DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]
    ///
    /// ``DELETE DATA`` update:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE DATA { <http://example.com> <http://example.com/p> "1" }')
    /// >>> list(store)
    /// []
    ///
    /// ``DELETE`` update:
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    /// >>> store.update('DELETE WHERE { <http://example.com> ?p ?o }')
    /// >>> list(store)
    /// []
    #[pyo3(text_signature = "($self, update, *, base_iri)")]
    #[args(update, "*", base_iri = "None")]
    fn update(&self, update: &str, base_iri: Option<&str>) -> PyResult<()> {
        let update = Update::parse(update, base_iri).map_err(|e| map_evaluation_error(e.into()))?;
        self.inner.update(update).map_err(map_evaluation_error)
    }

    /// Loads an RDF serialization into the store.
    ///
    /// Loads are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.
    /// The :py:func:`bulk_load` method is also available for much faster loading of big files but without transactional guarantees.
    ///
    /// Beware, the full file is loaded into memory.
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)
    ///
    /// It supports also some MIME type aliases.
    /// For example ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param input: The binary I/O object to read from. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: io.RawIOBase or io.BufferedIOBase
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which store the triples. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises IOError: if an I/O error happens during a quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(text_signature = "($self, data, /, mime_type, *, base_iri = None, to_graph = None)")]
    #[args(input, mime_type, "*", base_iri = "None", to_graph = "None")]
    fn load(
        &self,
        input: PyObject,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(PyGraphNameRef::try_from(graph_name)?)
        } else {
            None
        };
        let input = BufReader::new(PyFileLike::new(input));
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .load_graph(
                    input,
                    graph_format,
                    &to_graph_name.unwrap_or(PyGraphNameRef::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(PyValueError::new_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .load_dataset(input, dataset_format, base_iri)
                .map_err(map_io_err)
        } else {
            Err(PyValueError::new_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }

    /// Loads an RDF serialization into the store.
    ///
    /// This function is designed to be as fast as possible on big files without transactional guarantees.
    /// If the file is invalid only a piece of it might be written to the store.
    ///
    /// The :py:func:`load` method is also available for loads with transactional guarantees.
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)
    ///
    /// It supports also some MIME type aliases.
    /// For example ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param input: The binary I/O object to read from. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'rb')``.
    /// :type input: io.RawIOBase or io.BufferedIOBase
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    /// :type base_iri: str or None, optional
    /// :param to_graph: if it is a file composed of triples, the graph in which store the triples. By default, the default graph is used.
    /// :type to_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
    /// :raises SyntaxError: if the provided data is invalid.
    /// :raises IOError: if an I/O error happens during a quad insertion.
    ///
    /// >>> store = Store()
    /// >>> store.bulk_load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
    /// >>> list(store)
    /// [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
    #[pyo3(text_signature = "($self, data, /, mime_type, *, base_iri = None, to_graph = None)")]
    #[args(input, mime_type, "*", base_iri = "None", to_graph = "None")]
    fn bulk_load(
        &self,
        input: PyObject,
        mime_type: &str,
        base_iri: Option<&str>,
        to_graph: Option<&PyAny>,
    ) -> PyResult<()> {
        let to_graph_name = if let Some(graph_name) = to_graph {
            Some(PyGraphNameRef::try_from(graph_name)?)
        } else {
            None
        };
        let input = BufReader::new(PyFileLike::new(input));
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .bulk_load_graph(
                    input,
                    graph_format,
                    &to_graph_name.unwrap_or(PyGraphNameRef::DefaultGraph),
                    base_iri,
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if to_graph_name.is_some() {
                return Err(PyValueError::new_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .bulk_load_dataset(input, dataset_format, base_iri)
                .map_err(map_io_err)
        } else {
            Err(PyValueError::new_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }

    /// Dumps the store quads or triples into a file.
    ///
    /// It currently supports the following formats:
    ///
    /// * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    /// * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    /// * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    /// * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    /// * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)
    ///
    /// It supports also some MIME type aliases.
    /// For example ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    /// and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.
    ///
    /// :param output: The binary I/O object to write to. For example, it could be a file opened in binary mode with ``open('my_file.ttl', 'wb')``.
    /// :type input: io.RawIOBase or io.BufferedIOBase
    /// :param mime_type: the MIME type of the RDF serialization.
    /// :type mime_type: str
    /// :param from_graph: if a triple based format is requested, the store graph from which dump the triples. By default, the default graph is used.
    /// :type from_graph: NamedNode or BlankNode or DefaultGraph or None, optional
    /// :raises ValueError: if the MIME type is not supported or the `from_graph` parameter is given with a quad syntax.
    /// :raises IOError: if an I/O error happens during a quad lookup
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> output = io.BytesIO()
    /// >>> store.dump(output, "text/turtle", from_graph=NamedNode("http://example.com/g"))
    /// >>> output.getvalue()
    /// b'<http://example.com> <http://example.com/p> "1" .\n'
    #[pyo3(text_signature = "($self, output, /, mime_type, *, from_graph = None)")]
    #[args(output, mime_type, "*", from_graph = "None")]
    fn dump(&self, output: PyObject, mime_type: &str, from_graph: Option<&PyAny>) -> PyResult<()> {
        let from_graph_name = if let Some(graph_name) = from_graph {
            Some(PyGraphNameRef::try_from(graph_name)?)
        } else {
            None
        };
        let output = PyFileLike::new(output);
        if let Some(graph_format) = GraphFormat::from_media_type(mime_type) {
            self.inner
                .dump_graph(
                    output,
                    graph_format,
                    &from_graph_name.unwrap_or(PyGraphNameRef::DefaultGraph),
                )
                .map_err(map_io_err)
        } else if let Some(dataset_format) = DatasetFormat::from_media_type(mime_type) {
            if from_graph_name.is_some() {
                return Err(PyValueError::new_err(
                    "The target graph name parameter is not available for dataset formats",
                ));
            }
            self.inner
                .dump_dataset(output, dataset_format)
                .map_err(map_io_err)
        } else {
            Err(PyValueError::new_err(format!(
                "Not supported MIME type: {}",
                mime_type
            )))
        }
    }

    /// Returns an iterator over all the store named graphs.
    ///
    /// :return: an iterator of the store graph names.
    /// :rtype: iter(NamedNode or BlankNode)
    /// :raises IOError: if an I/O error happens during the named graphs lookup.
    ///
    /// >>> store = Store()
    /// >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[pyo3(text_signature = "($self)")]
    fn named_graphs(&self) -> GraphNameIter {
        GraphNameIter {
            inner: self.inner.named_graphs(),
        }
    }

    /// Adds a named graph to the store.
    ///
    /// :param graph_name: the name of the name graph to add.
    /// :type graph_name: NamedNode or BlankNode
    /// :raises IOError: if an I/O error happens during the named graph insertion.
    ///
    /// >>> store = Store()
    /// >>> store.add_graph(NamedNode('http://example.com/g'))
    /// >>> list(store.named_graphs())
    /// [<NamedNode value=http://example.com/g>]
    #[pyo3(text_signature = "($self, graph_name)")]
    fn add_graph(&self, graph_name: &PyAny) -> PyResult<()> {
        match PyGraphNameRef::try_from(graph_name)? {
            PyGraphNameRef::DefaultGraph => Ok(()),
            PyGraphNameRef::NamedNode(graph_name) => self
                .inner
                .insert_named_graph(&PyNamedOrBlankNodeRef::NamedNode(graph_name))
                .map(|_| ()),
            PyGraphNameRef::BlankNode(graph_name) => self
                .inner
                .insert_named_graph(&PyNamedOrBlankNodeRef::BlankNode(graph_name))
                .map(|_| ()),
        }
        .map_err(map_io_err)
    }

    /// Removes a graph from the store.
    ///
    /// The default graph will not be remove but just cleared.
    ///
    /// :param graph_name: the name of the name graph to remove.
    /// :type graph_name: NamedNode or BlankNode or DefaultGraph
    /// :raises IOError: if an I/O error happens during the named graph removal.
    ///
    /// >>> store = Store()
    /// >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    /// >>> store.remove_graph(NamedNode('http://example.com/g'))
    /// >>> list(store)
    /// []
    #[pyo3(text_signature = "($self, graph_name)")]
    fn remove_graph(&self, graph_name: &PyAny) -> PyResult<()> {
        match PyGraphNameRef::try_from(graph_name)? {
            PyGraphNameRef::DefaultGraph => self.inner.clear_graph(GraphNameRef::DefaultGraph),
            PyGraphNameRef::NamedNode(graph_name) => self
                .inner
                .remove_named_graph(&PyNamedOrBlankNodeRef::NamedNode(graph_name))
                .map(|_| ()),
            PyGraphNameRef::BlankNode(graph_name) => self
                .inner
                .remove_named_graph(&PyNamedOrBlankNodeRef::BlankNode(graph_name))
                .map(|_| ()),
        }
        .map_err(map_io_err)?;
        Ok(())
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }

    fn __bool__(&self) -> PyResult<bool> {
        Ok(!self.inner.is_empty()?)
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self.inner.len()?)
    }

    fn __contains__(&self, quad: PyQuad) -> PyResult<bool> {
        self.inner.contains(&quad).map_err(map_io_err)
    }

    fn __iter__(&self) -> QuadIter {
        QuadIter {
            inner: self.inner.iter(),
        }
    }
}

#[pyclass(unsendable, module = "oxigraph")]
pub struct QuadIter {
    inner: store::QuadIter,
}

#[pymethods]
impl QuadIter {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self) -> PyResult<Option<PyQuad>> {
        self.inner
            .next()
            .map(|q| Ok(q.map_err(map_io_err)?.into()))
            .transpose()
    }
}

#[pyclass(unsendable, module = "oxigraph")]
pub struct GraphNameIter {
    inner: store::GraphNameIter,
}

#[pymethods]
impl GraphNameIter {
    fn __iter__(slf: PyRef<'_, Self>) -> Py<Self> {
        slf.into()
    }

    fn __next__(&mut self) -> PyResult<Option<PyNamedOrBlankNode>> {
        self.inner
            .next()
            .map(|q| Ok(q.map_err(map_io_err)?.into()))
            .transpose()
    }
}

pub fn extract_quads_pattern<'a>(
    subject: &'a PyAny,
    predicate: &'a PyAny,
    object: &'a PyAny,
    graph_name: Option<&'a PyAny>,
) -> PyResult<(
    Option<PySubjectRef<'a>>,
    Option<PyNamedNodeRef<'a>>,
    Option<PyTermRef<'a>>,
    Option<PyGraphNameRef<'a>>,
)> {
    Ok((
        if subject.is_none() {
            None
        } else {
            Some(TryFrom::try_from(subject)?)
        },
        if predicate.is_none() {
            None
        } else {
            Some(TryFrom::try_from(predicate)?)
        },
        if object.is_none() {
            None
        } else {
            Some(TryFrom::try_from(object)?)
        },
        if let Some(graph_name) = graph_name {
            if graph_name.is_none() {
                None
            } else {
                Some(TryFrom::try_from(graph_name)?)
            }
        } else {
            None
        },
    ))
}
