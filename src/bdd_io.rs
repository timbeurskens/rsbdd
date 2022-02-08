extern crate dot;

use std::io::Write;
use std::borrow::Cow;
use crate::bdd::*;

impl<S: BDDSymbol> BDD<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) {
        dot::render(self, writer).unwrap()
    }
}

type GraphEdge<'a, S> = (&'a BDD<S>, bool, &'a BDD<S>);
type GraphNode<'a, S> = &'a BDD<S>;


impl<'a, S: BDDSymbol> dot::Labeller<'a, GraphNode<'a, S>, GraphEdge<'a, S>> for BDD<S> {
    fn graph_id(&'a self) -> dot::Id<'a> {
        dot::Id::new("bdd_graph").unwrap()
    }

    fn node_id(&'a self, n: &GraphNode<'a, S>) -> dot::Id<'a> {
        match n {
            &BDD::True => dot::Id::new("n_true").unwrap(),
            &BDD::False => dot::Id::new("n_false").unwrap(),
            &BDD::Choice(_, _, _) => dot::Id::new(format!("n_{:p}", *n)).unwrap(),
        }
    }

    fn node_label(&'a self, n: &GraphNode<'a, S>) -> dot::LabelText<'a> {
        match n {
            &BDD::True => dot::LabelText::label("true"),
            &BDD::False => dot::LabelText::label("false"),
            &BDD::Choice(_, v, _) => dot::LabelText::label(format!("{}", v)),
        }
    }

    fn edge_label(&'a self, (_, e, _): &GraphEdge<'a, S>) -> dot::LabelText<'a> {
        if *e {
            dot::LabelText::LabelStr(Cow::Borrowed("T"))
        } else {
            dot::LabelText::LabelStr(Cow::Borrowed("F"))
        }
    }
}

impl<'a, S: BDDSymbol> dot::GraphWalk<'a, GraphNode<'a, S>, GraphEdge<'a, S>> for BDD<S> {
    fn nodes(&'a self) -> dot::Nodes<'a, GraphNode<'a, S>> {
        match self {
            &BDD::Choice(ref l, _, ref r) => l.nodes().iter().chain(&[self]).chain(r.nodes().iter()).cloned().collect(),
            &BDD::True => Cow::Owned(vec![self]),
            &BDD::False => Cow::Owned(vec![self]),
        }
    }

    fn edges(&'a self) -> dot::Edges<'a, GraphEdge<'a, S>> {
        match self {
            &BDD::Choice(ref l, _, ref r) => 
                l.edges().iter()
                    .chain(r.edges().iter())
                    .chain(&[(self, true, l.as_ref()), (self, false, r.as_ref())])
                    .cloned().collect(),
            &BDD::True => Cow::Owned(vec![]),
            &BDD::False => Cow::Owned(vec![]),
        }
    }

    fn source(&self, (a, _, _): &GraphEdge<'a, S>) -> GraphNode<'a, S> {
        a
    }

    fn target(&self, (_, _, b): &GraphEdge<'a, S>) -> GraphNode<'a, S> {
        b
    }
}