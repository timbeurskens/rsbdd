extern crate dot;

use crate::bdd::*;
use itertools::Itertools;
use std::borrow::Cow;
use std::io;
use std::io::Write;
use std::rc::Rc;

type GraphEdge<S> = (Rc<BDD<S>>, bool, Rc<BDD<S>>);
type GraphNode<S> = Rc<BDD<S>>;

pub struct BDDGraph<S: BDDSymbol> {
    root: Rc<BDD<S>>,
    filter: TruthTableEntry,
}

impl<S: BDDSymbol> BDDGraph<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    pub fn new(root: &Rc<BDD<S>>, filter: TruthTableEntry) -> Self {
        BDDGraph {
            root: root.clone(),
            filter,
        }
    }
}

impl<'a, S: BDDSymbol> dot::Labeller<'a, GraphNode<S>, GraphEdge<S>> for BDDGraph<S> {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("bdd_graph").unwrap()
    }

    fn node_id(&self, n: &GraphNode<S>) -> dot::Id<'a> {
        match n.as_ref() {
            // use grep -v n_true or grep -v n_false to filter nodes adjacent to true or false
            BDD::True => dot::Id::new("n_true".to_string()).unwrap(),
            BDD::False => dot::Id::new("n_false".to_string()).unwrap(),
            _ => dot::Id::new(format!("n_{:p}", Rc::into_raw(n.clone()))).unwrap(),
            // _ => dot::Id::new(format!("n_{}", n.get_hash())).unwrap(), // use the hash for optimal sharing, use (above) pointers to test issue with duplicates
        }
    }

    fn node_label(&self, n: &GraphNode<S>) -> dot::LabelText<'a> {
        match n.as_ref() {
            BDD::True => dot::LabelText::label("true"),
            BDD::False => dot::LabelText::label("false"),
            &BDD::Choice(_, ref v, _) => dot::LabelText::label(format!("{}", v)),
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
    fn nodes_recursive(&self, root: Rc<BDD<S>>) -> dot::Nodes<'a, GraphNode<S>> {
        match root.as_ref() {
            &BDD::Choice(ref l, _, ref r) => {
                let l_nodes = self.nodes_recursive(l.clone());
                let r_nodes = self.nodes_recursive(r.clone());

                l_nodes
                    .iter()
                    .chain(&vec![root.clone()])
                    .chain(r_nodes.iter())
                    .unique()
                    .cloned()
                    .collect()
            }
            c if (self.filter == TruthTableEntry::Any)
                || (self.filter == TruthTableEntry::True && *c == BDD::True)
                || (self.filter == TruthTableEntry::False && *c == BDD::False) =>
            {
                vec![root.clone()].into()
            }
            _ => vec![].into(),
        }
    }

    fn edges_recursive(&self, root: Rc<BDD<S>>) -> dot::Edges<'a, GraphEdge<S>> {
        match root.as_ref() {
            &BDD::Choice(ref l, _, ref r) => {
                let l_edges = self.edges_recursive(l.clone());
                let r_edges = self.edges_recursive(r.clone());

                let mut self_edges = Vec::with_capacity(2);

                if (self.filter == TruthTableEntry::Any)
                    || (l.as_ref() != &BDD::True && l.as_ref() != &BDD::False)
                    || (l.as_ref() == &BDD::True && self.filter == TruthTableEntry::True)
                    || (l.as_ref() == &BDD::False && self.filter == TruthTableEntry::False)
                {
                    self_edges.push((root.clone(), true, l.clone()));
                }

                if (self.filter == TruthTableEntry::Any)
                    || (r.as_ref() != &BDD::True && r.as_ref() != &BDD::False)
                    || (r.as_ref() == &BDD::True && self.filter == TruthTableEntry::True)
                    || (r.as_ref() == &BDD::False && self.filter == TruthTableEntry::False)
                {
                    self_edges.push((root.clone(), false, r.clone()));
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
