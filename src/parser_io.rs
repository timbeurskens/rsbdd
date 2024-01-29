extern crate dot;

use std::io;
use std::io::Write;

use itertools::Itertools;

use crate::parser::*;

pub struct SymbolicParseTree {
    pub internal_tree: SymbolicBDD,
    pub nodes: Vec<SymbolicBDD>,
}

type GraphNode = usize;
type GraphEdge = (usize, String, usize);

impl SymbolicParseTree {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    fn nodes_recursive(root: &SymbolicBDD) -> Vec<SymbolicBDD> {
        let this_node = vec![root.clone()];

        match root {
            SymbolicBDD::BinaryOp(_, l, r) => {
                let left_nodes = Self::nodes_recursive(l);
                let right_nodes = Self::nodes_recursive(r);

                left_nodes
                    .into_iter()
                    .chain(right_nodes)
                    .chain(this_node)
                    .collect()
            }
            SymbolicBDD::Quantifier(_, _, f)
            | SymbolicBDD::Not(f)
            | SymbolicBDD::FixedPoint(_, _, f) => {
                let new_nodes = Self::nodes_recursive(f);

                new_nodes.into_iter().chain(this_node).collect()
            }
            SymbolicBDD::CountableConst(_, f, _) => {
                let mut new_nodes: Vec<SymbolicBDD> = this_node;

                for subtree in f {
                    new_nodes.extend(Self::nodes_recursive(subtree));
                }

                new_nodes
            }
            SymbolicBDD::CountableVariable(_, a, b) => {
                let mut new_nodes: Vec<SymbolicBDD> = this_node;

                for subtree in a {
                    new_nodes.extend(Self::nodes_recursive(subtree));
                }

                for subtree in b {
                    new_nodes.extend(Self::nodes_recursive(subtree));
                }

                new_nodes
            }
            SymbolicBDD::Ite(c, t, e) => {
                let mut new_nodes: Vec<SymbolicBDD> = this_node;

                new_nodes.extend(Self::nodes_recursive(c));
                new_nodes.extend(Self::nodes_recursive(t));
                new_nodes.extend(Self::nodes_recursive(e));

                new_nodes
            }
            SymbolicBDD::True
            | SymbolicBDD::False
            | SymbolicBDD::Var(_)
            | SymbolicBDD::Subtree(_)
            | SymbolicBDD::Reference(_) => this_node,
        }
    }

    pub fn new(src: &SymbolicBDD) -> Self {
        Self {
            internal_tree: src.clone(),
            nodes: Self::nodes_recursive(src).into_iter().unique().collect(),
        }
    }
}

impl<'a> dot::Labeller<'a, GraphNode, GraphEdge> for SymbolicParseTree {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("parse_tree").expect("cannot create Id named 'parse_tree'")
    }

    fn node_id(&self, n: &GraphNode) -> dot::Id<'a> {
        dot::Id::new(format!("n_{}", n))
            .unwrap_or_else(|_| panic!("cannot create Id named 'n_{n}'"))
    }

    fn node_label(&self, n: &GraphNode) -> dot::LabelText<'a> {
        match &self.nodes[*n] {
            SymbolicBDD::BinaryOp(ref op, _, _) => dot::LabelText::label(format!("{:?}", op)),
            SymbolicBDD::Quantifier(op, ref v, _) => dot::LabelText::label(format!(
                "{:?} [{}]",
                op,
                v.iter().map(|s| s.name.as_ref()).cloned().join(", ")
            )),
            SymbolicBDD::Not(_) => dot::LabelText::label("Not".to_string()),
            SymbolicBDD::CountableConst(ref v, _, n) => {
                dot::LabelText::label(format!("{:?} {}", v, n))
            }
            SymbolicBDD::CountableVariable(ref v, _, _) => {
                dot::LabelText::label(format!("{:?}", v))
            }
            SymbolicBDD::FixedPoint(ref v, init, _) => {
                if *init {
                    dot::LabelText::label(format!("GFP {}", v))
                } else {
                    dot::LabelText::label(format!("LFP {}", v))
                }
            }
            SymbolicBDD::Ite(_, _, _) => dot::LabelText::label("Ite".to_string()),
            SymbolicBDD::False => dot::LabelText::label("False".to_string()),
            SymbolicBDD::True => dot::LabelText::label("True".to_string()),
            SymbolicBDD::Var(v) => dot::LabelText::label(format!("Var {}", v)),
            SymbolicBDD::Subtree(_) => dot::LabelText::label("BDD".to_string()),
            SymbolicBDD::Reference(name) => dot::LabelText::label(format!("Ref {name}")),
        }
    }

    fn edge_label(&self, e: &GraphEdge) -> dot::LabelText<'a> {
        dot::LabelText::label(e.1.clone())
    }
}

impl<'a> dot::GraphWalk<'a, GraphNode, GraphEdge> for SymbolicParseTree {
    fn nodes(&self) -> dot::Nodes<'a, GraphNode> {
        (0..self.nodes.len()).collect()
    }

    fn edges(&self) -> dot::Edges<'a, GraphEdge> {
        let mut edges: Vec<GraphEdge> = Vec::new();

        for (i, node) in self.nodes.iter().enumerate() {
            match node {
                SymbolicBDD::BinaryOp(_, l, r) => {
                    edges.push((
                        i,
                        "L".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == l.as_ref())
                            .expect("cannot find position"),
                    ));
                    edges.push((
                        i,
                        "R".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == r.as_ref())
                            .expect("cannot find position"),
                    ));
                }
                SymbolicBDD::Quantifier(_, _, f)
                | SymbolicBDD::Not(f)
                | SymbolicBDD::FixedPoint(_, _, f) => {
                    edges.push((
                        i,
                        "".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == f.as_ref())
                            .expect("cannot find position"),
                    ));
                }
                SymbolicBDD::CountableConst(_, f, _) => {
                    for (j, subtree) in f.iter().enumerate() {
                        edges.push((
                            i,
                            format!("{{{}}}", j),
                            self.nodes
                                .iter()
                                .position(|n| n == subtree)
                                .expect("cannot find position"),
                        ));
                    }
                }
                SymbolicBDD::CountableVariable(_, a, b) => {
                    for (j, subtree) in a.iter().enumerate() {
                        edges.push((
                            i,
                            format!("L{{{}}}", j),
                            self.nodes
                                .iter()
                                .position(|n| n == subtree)
                                .expect("cannot find position"),
                        ));
                    }
                    for (j, subtree) in b.iter().enumerate() {
                        edges.push((
                            i,
                            format!("R{{{}}}", j),
                            self.nodes
                                .iter()
                                .position(|n| n == subtree)
                                .expect("cannot find position"),
                        ));
                    }
                }
                SymbolicBDD::Ite(c, t, e) => {
                    edges.push((
                        i,
                        "If".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == c.as_ref())
                            .expect("cannot find position"),
                    ));
                    edges.push((
                        i,
                        "Then".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == t.as_ref())
                            .expect("cannot find position"),
                    ));
                    edges.push((
                        i,
                        "Else".to_string(),
                        self.nodes
                            .iter()
                            .position(|n| n == e.as_ref())
                            .expect("cannot find position"),
                    ));
                }
                SymbolicBDD::False
                | SymbolicBDD::True
                | SymbolicBDD::Var(_)
                | SymbolicBDD::Subtree(_)
                | SymbolicBDD::Reference(_) => {}
            }
        }

        edges.into()
    }

    fn source(&self, e: &GraphEdge) -> GraphNode {
        e.0
    }

    fn target(&self, e: &GraphEdge) -> GraphNode {
        e.2
    }
}
