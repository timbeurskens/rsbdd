use std::io;
use std::io::Write;

use rustc_hash::FxHashSet;

use crate::{BDDSymbol, TruthTableEntry, bdd::*};

type GraphEdge<S> = (BDD<S>, bool, BDD<S>);
type GraphNode<S> = BDD<S>;

pub struct BDDGraph<S: BDDSymbol> {
    nodes: Vec<GraphNode<S>>,
    edges: Vec<GraphEdge<S>>,
}

impl<S: BDDSymbol> BDDGraph<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    pub fn new(root: &BDD<S>, filter: TruthTableEntry) -> Self {
        let keep = |b: &BDD<S>| {
            b.is_choice()
                || match filter {
                    TruthTableEntry::Any => true,
                    TruthTableEntry::True => b.is_true(),
                    TruthTableEntry::False => b.is_false(),
                    TruthTableEntry::None => false,
                }
        };

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        if !keep(root) {
            return Self { nodes, edges };
        }

        let mut visited = FxHashSet::default();
        visited.insert(root.get_hash());
        let mut stack = vec![root.clone()];

        while let Some(node) = stack.pop() {
            if let Node::Choice(l, _, r) = node.node() {
                for (e, child) in [(true, l), (false, r)].into_iter().filter(|(_, c)| keep(c)) {
                    edges.push((node.clone(), e, child.clone()));
                    if visited.insert(child.get_hash()) {
                        stack.push(child);
                    }
                }
            }
            nodes.push(node);
        }

        Self { nodes, edges }
    }
}

impl<'a, S: BDDSymbol> dot::Labeller<'a, GraphNode<S>, GraphEdge<S>> for BDDGraph<S> {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("bdd_graph").expect("cannot create Id named 'bdd_graph'")
    }

    fn node_id(&self, n: &GraphNode<S>) -> dot::Id<'a> {
        // use grep -v n_true or grep -v n_false to filter nodes adjacent to true or false
        let id = if n.is_true() {
            "n_true".to_string()
        } else if n.is_false() {
            "n_false".to_string()
        } else {
            format!("n_{}", n.get_hash())
        };
        dot::Id::new(id.clone()).unwrap_or_else(|_| panic!("cannot create Id named '{id}'"))
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
        dot::LabelText::label(if *e { "T" } else { "F" })
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
        self.nodes.as_slice().into()
    }

    fn edges(&'a self) -> dot::Edges<'a, GraphEdge<S>> {
        self.edges.as_slice().into()
    }

    fn source(&self, (a, _, _): &GraphEdge<S>) -> GraphNode<S> {
        a.clone()
    }

    fn target(&self, (_, _, b): &GraphEdge<S>) -> GraphNode<S> {
        b.clone()
    }
}
