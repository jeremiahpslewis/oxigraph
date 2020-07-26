//! RDF quads storage implementations.
//!
//! They encode a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
//! and allow querying and updating them using SPARQL.

pub mod memory;
pub(crate) mod numeric_encoder;
#[cfg(feature = "rocksdb")]
pub mod rocksdb;
#[cfg(feature = "sled")]
pub mod sled;

pub use crate::store::memory::MemoryStore;
#[cfg(feature = "rocksdb")]
pub use crate::store::rocksdb::RocksDbStore;
#[cfg(feature = "sled")]
pub use crate::store::sled::SledStore;

use crate::model::*;
use crate::store::numeric_encoder::*;
use crate::{DatasetSyntax, Error, GraphSyntax, Result};
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{
    NQuadsFormatter, NQuadsParser, NTriplesFormatter, NTriplesParser, TriGFormatter, TriGParser,
    TurtleFormatter, TurtleParser,
};
use rio_xml::{RdfXmlFormatter, RdfXmlParser};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::iter::Iterator;

pub(crate) trait ReadableEncodedStore: StrLookup {
    type QuadsIter: Iterator<Item = Result<EncodedQuad>> + 'static;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Self::QuadsIter;
}

pub(crate) trait WritableEncodedStore: StrContainer {
    type Error: From<<Self as StrContainer>::Error> + std::error::Error + Into<Error>;

    fn insert_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> std::result::Result<(), <Self as WritableEncodedStore>::Error>;

    fn remove_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> std::result::Result<(), <Self as WritableEncodedStore>::Error>;
}

fn load_graph<S: WritableEncodedStore>(
    store: &mut S,
    reader: impl BufRead,
    syntax: GraphSyntax,
    to_graph_name: &GraphName,
    base_iri: Option<&str>,
) -> Result<()> {
    let base_iri = base_iri.unwrap_or("");
    match syntax {
        GraphSyntax::NTriples => {
            load_from_triple_parser(store, NTriplesParser::new(reader)?, to_graph_name)
        }
        GraphSyntax::Turtle => {
            load_from_triple_parser(store, TurtleParser::new(reader, base_iri)?, to_graph_name)
        }
        GraphSyntax::RdfXml => {
            load_from_triple_parser(store, RdfXmlParser::new(reader, base_iri)?, to_graph_name)
        }
    }
}

fn load_from_triple_parser<S: WritableEncodedStore, P: TriplesParser>(
    store: &mut S,
    mut parser: P,
    to_graph_name: &GraphName,
) -> Result<()>
where
    Error: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    let to_graph_name = store
        .encode_graph_name(to_graph_name)
        .map_err(|e| e.into())?;
    parser.parse_all(&mut move |t| {
        let quad = store
            .encode_rio_triple_in_graph(t, to_graph_name, &mut bnode_map)
            .map_err(|e| e.into())?;
        store.insert_encoded(&quad).map_err(|e| e.into())
    })
}

fn dump_graph(
    triples: impl Iterator<Item = Result<Triple>>,
    writer: &mut impl Write,
    syntax: GraphSyntax,
) -> Result<()> {
    match syntax {
        GraphSyntax::NTriples => {
            let mut formatter = NTriplesFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish();
        }
        GraphSyntax::Turtle => {
            let mut formatter = TurtleFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish()?;
        }
        GraphSyntax::RdfXml => {
            let mut formatter = RdfXmlFormatter::new(writer)?;
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish()?;
        }
    }
    Ok(())
}

fn load_dataset<S: WritableEncodedStore>(
    store: &mut S,
    reader: impl BufRead,
    syntax: DatasetSyntax,
    base_iri: Option<&str>,
) -> Result<()> {
    let base_iri = base_iri.unwrap_or("");
    match syntax {
        DatasetSyntax::NQuads => load_from_quad_parser(store, NQuadsParser::new(reader)?),
        DatasetSyntax::TriG => load_from_quad_parser(store, TriGParser::new(reader, base_iri)?),
    }
}

fn load_from_quad_parser<S: WritableEncodedStore, P: QuadsParser>(
    store: &mut S,
    mut parser: P,
) -> Result<()>
where
    Error: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    parser.parse_all(&mut move |q| {
        let quad = store
            .encode_rio_quad(q, &mut bnode_map)
            .map_err(|e| e.into())?;
        store.insert_encoded(&quad).map_err(|e| e.into())
    })
}

fn dump_dataset(
    quads: impl Iterator<Item = Result<Quad>>,
    writer: &mut impl Write,
    syntax: DatasetSyntax,
) -> Result<()> {
    match syntax {
        DatasetSyntax::NQuads => {
            let mut formatter = NQuadsFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish();
        }
        DatasetSyntax::TriG => {
            let mut formatter = TriGFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish()?;
        }
    }
    Ok(())
}
