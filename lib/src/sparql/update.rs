use crate::error::invalid_input_error;
use crate::io::{GraphFormat, GraphParser};
use crate::model::{
    BlankNode as OxBlankNode, GraphName as OxGraphName, GraphNameRef, Literal as OxLiteral,
    NamedNode as OxNamedNode, NamedNodeRef, Quad as OxQuad, Term as OxTerm, Triple as OxTriple,
};
use crate::sparql::algebra::QueryDataset;
use crate::sparql::dataset::DatasetView;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::http::Client;
use crate::sparql::plan::EncodedTuple;
use crate::sparql::plan_builder::PlanBuilder;
use crate::sparql::{EvaluationError, Update, UpdateOptions};
use crate::storage::numeric_encoder::{Decoder, EncodedTerm};
use crate::storage::{Storage, StorageWriter};
use oxiri::Iri;
use spargebra::algebra::{GraphPattern, GraphTarget};
use spargebra::term::{
    BlankNode, GraphName, GraphNamePattern, GroundQuad, GroundQuadPattern, GroundSubject,
    GroundTerm, GroundTermPattern, GroundTriple, GroundTriplePattern, Literal, NamedNode,
    NamedNodePattern, Quad, QuadPattern, Subject, Term, TermPattern, Triple, TriplePattern,
    Variable,
};
use spargebra::GraphUpdateOperation;
use std::collections::HashMap;
use std::io::{BufReader, Error, ErrorKind};
use std::rc::Rc;

pub fn evaluate_update(
    storage: &Storage,
    update: Update,
    options: UpdateOptions,
) -> Result<(), EvaluationError> {
    let base_iri = update.inner.base_iri.map(Rc::new);
    storage
        .transaction(move |transaction| {
            let client = Client::new(options.query_options.http_timeout);
            SimpleUpdateEvaluator {
                transaction,
                base_iri: base_iri.clone(),
                options: options.clone(),
                client,
            }
            .eval_all(&update.inner.operations, &update.using_datasets)
            .map_err(|e| match e {
                EvaluationError::Io(e) => e,
                q => Error::new(ErrorKind::Other, q),
            })
        })
        .map_err(|e| {
            if e.get_ref()
                .map_or(false, |inner| inner.is::<EvaluationError>())
            {
                *e.into_inner().unwrap().downcast().unwrap()
            } else {
                EvaluationError::Io(e)
            }
        })?;
    Ok(())
}

struct SimpleUpdateEvaluator<'a> {
    transaction: StorageWriter<'a>,
    base_iri: Option<Rc<Iri<String>>>,
    options: UpdateOptions,
    client: Client,
}

impl SimpleUpdateEvaluator<'_> {
    fn eval_all(
        &mut self,
        updates: &[GraphUpdateOperation],
        using_datasets: &[Option<QueryDataset>],
    ) -> Result<(), EvaluationError> {
        for (update, using_dataset) in updates.iter().zip(using_datasets) {
            self.eval(update, using_dataset)?;
        }
        Ok(())
    }

    fn eval(
        &mut self,
        update: &GraphUpdateOperation,
        using_dataset: &Option<QueryDataset>,
    ) -> Result<(), EvaluationError> {
        match update {
            GraphUpdateOperation::InsertData { data } => self.eval_insert_data(data),
            GraphUpdateOperation::DeleteData { data } => self.eval_delete_data(data),
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                pattern,
                ..
            } => self.eval_delete_insert(delete, insert, using_dataset.as_ref().unwrap(), pattern),
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                if let Err(error) = self.eval_load(source, destination) {
                    if *silent {
                        Ok(())
                    } else {
                        Err(error)
                    }
                } else {
                    Ok(())
                }
            }
            GraphUpdateOperation::Clear { graph, silent } => self.eval_clear(graph, *silent),
            GraphUpdateOperation::Create { graph, silent } => self.eval_create(graph, *silent),
            GraphUpdateOperation::Drop { graph, silent } => self.eval_drop(graph, *silent),
        }
    }

    fn eval_insert_data(&mut self, data: &[Quad]) -> Result<(), EvaluationError> {
        let mut bnodes = HashMap::new();
        for quad in data {
            let quad = Self::convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref())?;
        }
        Ok(())
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) -> Result<(), EvaluationError> {
        for quad in data {
            let quad = Self::convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref())?;
        }
        Ok(())
    }

    fn eval_delete_insert(
        &mut self,
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        using: &QueryDataset,
        algebra: &GraphPattern,
    ) -> Result<(), EvaluationError> {
        let dataset = Rc::new(DatasetView::new(self.transaction.reader(), using));
        let (plan, variables) = PlanBuilder::build(
            dataset.as_ref(),
            algebra,
            false,
            &self.options.query_options.custom_functions,
        )?;
        let evaluator = SimpleEvaluator::new(
            dataset.clone(),
            self.base_iri.clone(),
            self.options.query_options.service_handler(),
            Rc::new(self.options.query_options.custom_functions.clone()),
        );
        let mut bnodes = HashMap::new();
        for tuple in evaluator.plan_evaluator(&plan)(EncodedTuple::with_capacity(variables.len())) {
            let tuple = tuple?;
            for quad in delete {
                if let Some(quad) =
                    Self::convert_ground_quad_pattern(quad, &variables, &tuple, &dataset)?
                {
                    self.transaction.remove(quad.as_ref())?;
                }
            }
            for quad in insert {
                if let Some(quad) =
                    Self::convert_quad_pattern(quad, &variables, &tuple, &dataset, &mut bnodes)?
                {
                    self.transaction.insert(quad.as_ref())?;
                }
            }
            bnodes.clear();
        }
        Ok(())
    }

    fn eval_load(&mut self, from: &NamedNode, to: &GraphName) -> Result<(), EvaluationError> {
        let (content_type, body) = self.client.get(
            &from.iri,
            "application/n-triples, text/turtle, application/rdf+xml",
        )?;
        let format = GraphFormat::from_media_type(&content_type).ok_or_else(|| {
            EvaluationError::msg(format!(
                "Unsupported Content-Type returned by {}: {}",
                from, content_type
            ))
        })?;
        let to_graph_name = match to {
            GraphName::NamedNode(graph_name) => NamedNodeRef::new_unchecked(&graph_name.iri).into(),
            GraphName::DefaultGraph => GraphNameRef::DefaultGraph,
        };
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = &self.base_iri {
            parser = parser
                .with_base_iri(base_iri.as_str())
                .map_err(invalid_input_error)?;
        }
        for t in parser.read_triples(BufReader::new(body))? {
            self.transaction
                .insert(t?.as_ref().in_graph(to_graph_name))?;
        }
        Ok(())
    }

    fn eval_create(&mut self, graph_name: &NamedNode, silent: bool) -> Result<(), EvaluationError> {
        let graph_name = NamedNodeRef::new_unchecked(&graph_name.iri);
        if self.transaction.insert_named_graph(graph_name.into())? || silent {
            Ok(())
        } else {
            Err(EvaluationError::msg(format!(
                "The graph {} already exists",
                graph_name
            )))
        }
    }

    fn eval_clear(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                let graph_name = NamedNodeRef::new_unchecked(&graph_name.iri);
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.into())?
                {
                    Ok(self.transaction.clear_graph(graph_name.into())?)
                } else if silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph
                    )))
                }
            }
            GraphTarget::DefaultGraph => {
                self.transaction.clear_graph(GraphNameRef::DefaultGraph)?;
                Ok(())
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.clear_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear_all_graphs()?),
        }
    }

    fn eval_drop(&mut self, graph: &GraphTarget, silent: bool) -> Result<(), EvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                let graph_name = NamedNodeRef::new_unchecked(&graph_name.iri);
                if self.transaction.remove_named_graph(graph_name.into())? || silent {
                    Ok(())
                } else {
                    Err(EvaluationError::msg(format!(
                        "The graph {} does not exists",
                        graph_name
                    )))
                }
            }
            GraphTarget::DefaultGraph => {
                Ok(self.transaction.clear_graph(GraphNameRef::DefaultGraph)?)
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.remove_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear()?),
        }
    }

    fn convert_quad(quad: &Quad, bnodes: &mut HashMap<BlankNode, OxBlankNode>) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                Subject::NamedNode(subject) => Self::convert_named_node(subject).into(),
                Subject::BlankNode(subject) => Self::convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            predicate: Self::convert_named_node(&quad.predicate),
            object: match &quad.object {
                Term::NamedNode(object) => Self::convert_named_node(object).into(),
                Term::BlankNode(object) => Self::convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => Self::convert_literal(object).into(),
                Term::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => Self::convert_named_node(graph_name).into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_triple(triple: &Triple, bnodes: &mut HashMap<BlankNode, OxBlankNode>) -> OxTriple {
        OxTriple {
            subject: match &triple.subject {
                Subject::NamedNode(subject) => Self::convert_named_node(subject).into(),
                Subject::BlankNode(subject) => Self::convert_blank_node(subject, bnodes).into(),
                Subject::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
            predicate: Self::convert_named_node(&triple.predicate),
            object: match &triple.object {
                Term::NamedNode(object) => Self::convert_named_node(object).into(),
                Term::BlankNode(object) => Self::convert_blank_node(object, bnodes).into(),
                Term::Literal(object) => Self::convert_literal(object).into(),
                Term::Triple(subject) => Self::convert_triple(subject, bnodes).into(),
            },
        }
    }

    fn convert_named_node(node: &NamedNode) -> OxNamedNode {
        OxNamedNode::new_unchecked(&node.iri)
    }

    fn convert_blank_node(
        node: &BlankNode,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> OxBlankNode {
        bnodes.entry(node.clone()).or_default().clone()
    }

    fn convert_literal(literal: &Literal) -> OxLiteral {
        match literal {
            Literal::Simple { value } => OxLiteral::new_simple_literal(value),
            Literal::LanguageTaggedString { value, language } => {
                OxLiteral::new_language_tagged_literal_unchecked(value, language)
            }
            Literal::Typed { value, datatype } => {
                OxLiteral::new_typed_literal(value, NamedNodeRef::new_unchecked(&datatype.iri))
            }
        }
    }

    fn convert_ground_quad(quad: &GroundQuad) -> OxQuad {
        OxQuad {
            subject: match &quad.subject {
                GroundSubject::NamedNode(subject) => Self::convert_named_node(subject).into(),
                GroundSubject::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            predicate: Self::convert_named_node(&quad.predicate),
            object: match &quad.object {
                GroundTerm::NamedNode(object) => Self::convert_named_node(object).into(),
                GroundTerm::Literal(object) => Self::convert_literal(object).into(),
                GroundTerm::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            graph_name: match &quad.graph_name {
                GraphName::NamedNode(graph_name) => Self::convert_named_node(graph_name).into(),
                GraphName::DefaultGraph => OxGraphName::DefaultGraph,
            },
        }
    }

    fn convert_ground_triple(triple: &GroundTriple) -> OxTriple {
        OxTriple {
            subject: match &triple.subject {
                GroundSubject::NamedNode(subject) => Self::convert_named_node(subject).into(),
                GroundSubject::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
            predicate: Self::convert_named_node(&triple.predicate),
            object: match &triple.object {
                GroundTerm::NamedNode(object) => Self::convert_named_node(object).into(),
                GroundTerm::Literal(object) => Self::convert_literal(object).into(),
                GroundTerm::Triple(subject) => Self::convert_ground_triple(subject).into(),
            },
        }
    }

    fn convert_quad_pattern(
        quad: &QuadPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxQuad>, EvaluationError> {
        Ok(Some(OxQuad {
            subject: match Self::convert_term_or_var(
                &quad.subject,
                variables,
                values,
                dataset,
                bnodes,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                Self::convert_named_node_or_var(&quad.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                Self::convert_term_or_var(&quad.object, variables, values, dataset, bnodes)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) =
                Self::convert_graph_name_or_var(&quad.graph_name, variables, values, dataset)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_term_or_var(
        term: &TermPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxTerm>, EvaluationError> {
        Ok(match term {
            TermPattern::NamedNode(term) => Some(Self::convert_named_node(term).into()),
            TermPattern::BlankNode(bnode) => Some(Self::convert_blank_node(bnode, bnodes).into()),
            TermPattern::Literal(term) => Some(Self::convert_literal(term).into()),
            TermPattern::Triple(triple) => {
                Self::convert_triple_pattern(triple, variables, values, dataset, bnodes)?
                    .map(|t| t.into())
            }
            TermPattern::Variable(v) => Self::lookup_variable(v, variables, values)
                .map(|node| dataset.decode_term(&node))
                .transpose()?,
        })
    }

    fn convert_named_node_or_var(
        term: &NamedNodePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxNamedNode>, EvaluationError> {
        Ok(match term {
            NamedNodePattern::NamedNode(term) => Some(Self::convert_named_node(term)),
            NamedNodePattern::Variable(v) => Self::lookup_variable(v, variables, values)
                .map(|node| dataset.decode_named_node(&node))
                .transpose()?,
        })
    }

    fn convert_graph_name_or_var(
        term: &GraphNamePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxGraphName>, EvaluationError> {
        match term {
            GraphNamePattern::NamedNode(term) => Ok(Some(Self::convert_named_node(term).into())),
            GraphNamePattern::DefaultGraph => Ok(Some(OxGraphName::DefaultGraph)),
            GraphNamePattern::Variable(v) => Self::lookup_variable(v, variables, values)
                .map(|node| {
                    Ok(if node == EncodedTerm::DefaultGraph {
                        OxGraphName::DefaultGraph
                    } else {
                        dataset.decode_named_node(&node)?.into()
                    })
                })
                .transpose(),
        }
    }

    fn convert_triple_pattern(
        triple: &TriplePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
        bnodes: &mut HashMap<BlankNode, OxBlankNode>,
    ) -> Result<Option<OxTriple>, EvaluationError> {
        Ok(Some(OxTriple {
            subject: match Self::convert_term_or_var(
                &triple.subject,
                variables,
                values,
                dataset,
                bnodes,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                Self::convert_named_node_or_var(&triple.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                Self::convert_term_or_var(&triple.object, variables, values, dataset, bnodes)?
            {
                object
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_ground_quad_pattern(
        quad: &GroundQuadPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxQuad>, EvaluationError> {
        Ok(Some(OxQuad {
            subject: match Self::convert_ground_term_or_var(
                &quad.subject,
                variables,
                values,
                dataset,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                Self::convert_named_node_or_var(&quad.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                Self::convert_ground_term_or_var(&quad.object, variables, values, dataset)?
            {
                object
            } else {
                return Ok(None);
            },
            graph_name: if let Some(graph_name) =
                Self::convert_graph_name_or_var(&quad.graph_name, variables, values, dataset)?
            {
                graph_name
            } else {
                return Ok(None);
            },
        }))
    }

    fn convert_ground_term_or_var(
        term: &GroundTermPattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxTerm>, EvaluationError> {
        Ok(match term {
            GroundTermPattern::NamedNode(term) => Some(Self::convert_named_node(term).into()),
            GroundTermPattern::Literal(term) => Some(Self::convert_literal(term).into()),
            GroundTermPattern::Triple(triple) => {
                Self::convert_ground_triple_pattern(triple, variables, values, dataset)?
                    .map(|t| t.into())
            }
            GroundTermPattern::Variable(v) => Self::lookup_variable(v, variables, values)
                .map(|node| dataset.decode_term(&node))
                .transpose()?,
        })
    }

    fn convert_ground_triple_pattern(
        triple: &GroundTriplePattern,
        variables: &[Variable],
        values: &EncodedTuple,
        dataset: &DatasetView,
    ) -> Result<Option<OxTriple>, EvaluationError> {
        Ok(Some(OxTriple {
            subject: match Self::convert_ground_term_or_var(
                &triple.subject,
                variables,
                values,
                dataset,
            )? {
                Some(OxTerm::NamedNode(node)) => node.into(),
                Some(OxTerm::BlankNode(node)) => node.into(),
                Some(OxTerm::Triple(triple)) => triple.into(),
                Some(OxTerm::Literal(_)) | None => return Ok(None),
            },
            predicate: if let Some(predicate) =
                Self::convert_named_node_or_var(&triple.predicate, variables, values, dataset)?
            {
                predicate
            } else {
                return Ok(None);
            },
            object: if let Some(object) =
                Self::convert_ground_term_or_var(&triple.object, variables, values, dataset)?
            {
                object
            } else {
                return Ok(None);
            },
        }))
    }

    fn lookup_variable(
        v: &Variable,
        variables: &[Variable],
        values: &EncodedTuple,
    ) -> Option<EncodedTerm> {
        variables
            .iter()
            .position(|v2| v == v2)
            .and_then(|i| values.get(i))
            .cloned()
    }
}
