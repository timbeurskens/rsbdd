extern crate dot;

use crate::parser::*;
use itertools::Itertools;
use std::boxed::Box;
use std::io;
use std::io::Write;

pub struct SymbolicParseTree {
    pub internal_tree: SymbolicBDD,
    pub nodes: Vec<Box<SymbolicBDD>>,
}

type GraphNode = usize;
type GraphEdge = (usize, String, usize);

impl SymbolicParseTree {
    pub fn render_dot<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        dot::render(self, writer)
    }

    fn nodes_recursive(root: &SymbolicBDD) -> Vec<Box<SymbolicBDD>> {
        let this_node = vec![Box::new(root.clone())];

        match root {
            SymbolicBDD::BinaryOp(_, l, r) => {
                let left_nodes = SymbolicParseTree::nodes_recursive(l);
                let right_nodes = SymbolicParseTree::nodes_recursive(r);

                left_nodes
                    .into_iter()
                    .chain(right_nodes)
                    .chain(this_node)
                    .collect()
            }
            SymbolicBDD::Quantifier(_, _, f) | SymbolicBDD::Not(f) => {
                let new_nodes = SymbolicParseTree::nodes_recursive(f);

                new_nodes.into_iter().chain(this_node).collect()
            }
            SymbolicBDD::CountableConst(_, f, _) => {
                let mut new_nodes: Vec<Box<SymbolicBDD>> = this_node;

                for subtree in f {
                    new_nodes.extend(SymbolicParseTree::nodes_recursive(subtree).into_iter());
                }

                new_nodes
            }
            SymbolicBDD::CountableVariable(_, a, b) => {
                let mut new_nodes: Vec<Box<SymbolicBDD>> = this_node;

                for subtree in a {
                    new_nodes.extend(SymbolicParseTree::nodes_recursive(subtree).into_iter());
                }

                for subtree in b {
                    new_nodes.extend(SymbolicParseTree::nodes_recursive(subtree).into_iter());
                }

                new_nodes
            }
            other => vec![Box::new(other.clone())],
        }
    }

    pub fn new(src: &SymbolicBDD) -> Self {
        SymbolicParseTree {
            internal_tree: src.clone(),
            nodes: SymbolicParseTree::nodes_recursive(src)
                .into_iter()
                .unique()
                .collect(),
        }
    }
}

impl<'a> dot::Labeller<'a, GraphNode, GraphEdge> for SymbolicParseTree {
    fn graph_id(&self) -> dot::Id<'a> {
        dot::Id::new("parse_tree").unwrap()
    }

    fn node_id(&self, n: &GraphNode) -> dot::Id<'a> {
        dot::Id::new(format!("n_{}", n)).unwrap()
    }

    fn node_label(&self, n: &GraphNode) -> dot::LabelText<'a> {
        match self.nodes[*n].as_ref() {
            SymbolicBDD::BinaryOp(ref op, _, _) => dot::LabelText::label(format!("{:?}", op)),
            SymbolicBDD::Quantifier(op, ref v, _) => {
                dot::LabelText::label(format!("{:?} {:?}", op, v))
            }
            SymbolicBDD::Not(_) => dot::LabelText::label("Not".to_string()),
            SymbolicBDD::CountableConst(ref v, _, n) => {
                dot::LabelText::label(format!("{:?} {}", v, n))
            }
            SymbolicBDD::CountableVariable(ref v, _, _) => {
                dot::LabelText::label(format!("{:?}", v))
            }
            SymbolicBDD::False => dot::LabelText::label("False".to_string()),
            SymbolicBDD::True => dot::LabelText::label("True".to_string()),
            SymbolicBDD::Var(v) => dot::LabelText::label(format!("Var {}", v)),
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
            match node.as_ref() {
                SymbolicBDD::BinaryOp(_, l, r) => {
                    edges.push((
                        i,
                        "L".to_string(),
                        self.nodes.iter().position(|n| n == l).unwrap(),
                    ));
                    edges.push((
                        i,
                        "R".to_string(),
                        self.nodes.iter().position(|n| n == r).unwrap(),
                    ));
                }
                SymbolicBDD::Quantifier(_, _, f) | SymbolicBDD::Not(f) => {
                    edges.push((
                        i,
                        "".to_string(),
                        self.nodes.iter().position(|n| n == f).unwrap(),
                    ));
                }
                SymbolicBDD::CountableConst(_, f, _) => {
                    for (j, subtree) in f.iter().enumerate() {
                        edges.push((
                            i,
                            format!("{{{}}}", j),
                            self.nodes
                                .iter()
                                .position(|n| n.as_ref() == subtree)
                                .unwrap(),
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
                                .position(|n| n.as_ref() == subtree)
                                .unwrap(),
                        ));
                    }
                    for (j, subtree) in b.iter().enumerate() {
                        edges.push((
                            i,
                            format!("R{{{}}}", j),
                            self.nodes
                                .iter()
                                .position(|n| n.as_ref() == subtree)
                                .unwrap(),
                        ));
                    }
                }
                _ => {}
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
