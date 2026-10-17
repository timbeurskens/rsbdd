extern crate dot;

use std::borrow::Cow;
use std::io;
use std::io::Write;

use rustc_hash::FxHashSet;

use crate::{BDDSymbol, TruthTableEntry, bdd::*};

type GraphEdge<S> = (BDD<S>, bool, BDD<S>);
type GraphNode<S> = BDD<S>;

pub struct BDDGraph<S: BDDSymbol> {
    nodes: Vec<GraphNode<S>>,
    edges: Vec<GraphEdge<S>>,
    filter: TruthTableEntry,
}

impl<S: BDDSymbol> BDDGraph<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    pub fn new(root: &BDD<S>, filter: TruthTableEntry) -> Self {
        let mut graph = Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            filter,
        };

        if graph.keep(root) {
            let mut visited = FxHashSet::default();
            visited.insert(root.get_hash());
            let mut stack = vec![root.clone()];

            while let Some(node) = stack.pop() {
                if let Node::Choice(l, _, r) = node.node() {
                    for (e, child) in [(true, l), (false, r)] {
                        if !graph.keep(&child) {
                            continue;
                        }
                        graph.edges.push((node.clone(), e, child.clone()));
                        if visited.insert(child.get_hash()) {
                            stack.push(child);
                        }
                    }
                }
                graph.nodes.push(node);
            }
        }

        graph
    }

    fn keep(&self, b: &BDD<S>) -> bool {
        b.is_choice()
            || self.filter == TruthTableEntry::Any
            || (self.filter == TruthTableEntry::True && b.is_true())
            || (self.filter == TruthTableEntry::False && b.is_false())
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

    fn node_shape(&'a self, n: &GraphNode<S>) -> Option<dot::LabelText<'a>> {
        n.is_const().then(|| dot::LabelText::label("box"))
    }

    fn edge_label(&self, (_, e, _): &GraphEdge<S>) -> dot::LabelText<'a> {
        if *e {
            dot::LabelText::LabelStr(Cow::Borrowed("T"))
        } else {
            dot::LabelText::LabelStr(Cow::Borrowed("F"))
        }
    }

    fn edge_style(&'a self, (_, e, _): &GraphEdge<S>) -> dot::Style {
        if *e {
            dot::Style::None
        } else {
            dot::Style::Dashed
        }
    }
}

impl<'a, S: BDDSymbol> dot::GraphWalk<'a, GraphNode<S>, GraphEdge<S>> for BDDGraph<S> {
    fn nodes(&'a self) -> dot::Nodes<'a, GraphNode<S>> {
        Cow::Borrowed(&self.nodes)
    }

    fn edges(&'a self) -> dot::Edges<'a, GraphEdge<S>> {
        Cow::Borrowed(&self.edges)
    }

    fn source(&self, (a, _, _): &GraphEdge<S>) -> GraphNode<S> {
        a.clone()
    }

    fn target(&self, (_, _, b): &GraphEdge<S>) -> GraphNode<S> {
        b.clone()
    }
}
