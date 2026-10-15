extern crate dot;

use std::borrow::Cow;
use std::io;
use std::io::Write;

use itertools::Itertools;

use crate::{BDDSymbol, TruthTableEntry, bdd::*};

type GraphEdge<S> = (BDD<S>, bool, BDD<S>);
type GraphNode<S> = BDD<S>;

pub struct BDDGraph<S: BDDSymbol> {
    root: BDD<S>,
    filter: TruthTableEntry,
}

impl<S: BDDSymbol> BDDGraph<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    pub fn new(root: &BDD<S>, filter: TruthTableEntry) -> Self {
        Self {
            root: root.clone(),
            filter,
        }
    }
}

impl<'a, S: BDDSymbol> dot::Labeller<'a, GraphNode<S>, GraphEdge<S>> for BDDGraph<S> {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("bdd_graph").expect("cannot create Id named 'bdd_graph'")
    }

    fn node_id(&self, n: &GraphNode<S>) -> dot::Id<'a> {
        match n.node() {
            // use grep -v n_true or grep -v n_false to filter nodes adjacent to true or false
            Node::True => {
                dot::Id::new("n_true".to_string()).expect("cannot create Id named 'n_true'")
            }
            Node::False => {
                dot::Id::new("n_false".to_string()).expect("cannot create Id named 'n_false'")
            }
            Node::Choice(..) => dot::Id::new(format!("n_{}", n.get_hash()))
                .unwrap_or_else(|_| panic!("cannot create Id named 'n_{}'", n.get_hash())),
        }
    }

    fn node_label(&self, n: &GraphNode<S>) -> dot::LabelText<'a> {
        match n.node() {
            Node::True => dot::LabelText::label("true"),
            Node::False => dot::LabelText::label("false"),
            Node::Choice(_, v, _) => dot::LabelText::label(format!("{}", v)),
        }
    }

    fn edge_label(&self, (_, e, _): &GraphEdge<S>) -> dot::LabelText<'a> {
        if *e {
            dot::LabelText::LabelStr(Cow::Borrowed("T"))
        } else {
            dot::LabelText::LabelStr(Cow::Borrowed("F"))
        }
    }
}

impl<'a, S: BDDSymbol> dot::GraphWalk<'a, GraphNode<S>, GraphEdge<S>> for BDDGraph<S> {
    fn nodes(&self) -> dot::Nodes<'a, GraphNode<S>> {
        self.nodes_recursive(self.root.clone())
    }

    fn edges(&self) -> dot::Edges<'a, GraphEdge<S>> {
        self.edges_recursive(self.root.clone())
    }

    fn source(&self, (a, _, _): &GraphEdge<S>) -> GraphNode<S> {
        a.clone()
    }

    fn target(&self, (_, _, b): &GraphEdge<S>) -> GraphNode<S> {
        b.clone()
    }
}

impl<'a, S: BDDSymbol> BDDGraph<S> {
    fn nodes_recursive(&self, root: BDD<S>) -> dot::Nodes<'a, GraphNode<S>> {
        match root.node() {
            Node::Choice(l, _, r) => {
                let l_nodes = self.nodes_recursive(l);
                let r_nodes = self.nodes_recursive(r);

                l_nodes
                    .iter()
                    .chain(&vec![root])
                    .chain(r_nodes.iter())
                    .unique()
                    .cloned()
                    .collect()
            }
            _ if (self.filter == TruthTableEntry::Any)
                || (self.filter == TruthTableEntry::True && root.is_true())
                || (self.filter == TruthTableEntry::False && root.is_false()) =>
            {
                vec![root].into()
            }
            _ => vec![].into(),
        }
    }

    fn edges_recursive(&self, root: BDD<S>) -> dot::Edges<'a, GraphEdge<S>> {
        match root.node() {
            Node::Choice(l, _, r) => {
                let l_edges = self.edges_recursive(l.clone());
                let r_edges = self.edges_recursive(r.clone());

                let mut self_edges = Vec::with_capacity(2);

                if (self.filter == TruthTableEntry::Any)
                    || l.is_choice()
                    || (l.is_true() && self.filter == TruthTableEntry::True)
                    || (l.is_false() && self.filter == TruthTableEntry::False)
                {
                    self_edges.push((root.clone(), true, l));
                }

                if (self.filter == TruthTableEntry::Any)
                    || r.is_choice()
                    || (r.is_true() && self.filter == TruthTableEntry::True)
                    || (r.is_false() && self.filter == TruthTableEntry::False)
                {
                    self_edges.push((root, false, r));
                }

                l_edges
                    .iter()
                    .chain(r_edges.iter())
                    .chain(self_edges.iter())
                    .unique() // disable unique edges when testing for duplicates
                    .cloned()
                    .collect()
            }
            _ => vec![].into(),
        }
    }
}
