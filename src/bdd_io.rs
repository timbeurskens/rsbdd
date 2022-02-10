extern crate dot;

use crate::bdd::*;
use itertools::Itertools;
use std::borrow::Cow;
use std::io::Write;
use std::rc::Rc;

// todo: currently the filter step: find a similar node in the environment, and filter duplicates reduces the graph significantly
// this should be done during the bdd computation instead (but how?)

type GraphEdge<S> = (Rc<BDD<S>>, bool, Rc<BDD<S>>);
type GraphNode<S> = Rc<BDD<S>>;

pub struct BDDGraph<S: BDDSymbol> {
    env: Rc<BDDEnv<S>>,
    root: Rc<BDD<S>>,
}

impl<S: BDDSymbol> BDDGraph<S> {
    pub fn render_dot<W: Write>(&self, writer: &mut W) {
        dot::render(self, writer).unwrap()
    }

    pub fn new(env: &Rc<BDDEnv<S>>, root: &Rc<BDD<S>>) -> Self {
        BDDGraph {
            env: env.clone(),
            root: root.clone(),
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
            &BDD::True => dot::Id::new(format!("n_true")).unwrap(),
            &BDD::False => dot::Id::new(format!("n_false")).unwrap(),
            _ => dot::Id::new(format!("n_{:p}", n.as_ref())).unwrap(),
        }
    }

    fn node_label(&self, n: &GraphNode<S>) -> dot::LabelText<'a> {
        match n.as_ref() {
            &BDD::True => dot::LabelText::label("true"),
            &BDD::False => dot::LabelText::label("false"),
            &BDD::Choice(_, v, _) => dot::LabelText::label(format!("{}", v)),
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
        self.nodes_recursive(&self.root)
    }

    fn edges(&self) -> dot::Edges<'a, GraphEdge<S>> {
        self.edges_recursive(&self.root)
    }

    fn source(&self, (a, _, _): &GraphEdge<S>) -> GraphNode<S> {
        a.clone()
    }

    fn target(&self, (_, _, b): &GraphEdge<S>) -> GraphNode<S> {
        b.clone()
    }
}

impl<'a, S: BDDSymbol> BDDGraph<S> {
    fn nodes_recursive(&self, root: &Rc<BDD<S>>) -> dot::Nodes<'a, GraphNode<S>> {
        let _root = self.env.find(root);

        match _root.as_ref() {
            &BDD::Choice(ref l, _, ref r) => {
                let l_nodes = self.nodes_recursive(l);
                let r_nodes = self.nodes_recursive(r);

                l_nodes
                    .iter()
                    .chain(&vec![_root.clone()])
                    .chain(r_nodes.iter())
                    .unique()
                    .cloned()
                    .collect()
            }
            &BDD::True | &BDD::False => vec![_root.clone()].into(),
        }
    }

    fn edges_recursive(&self, root: &Rc<BDD<S>>) -> dot::Edges<'a, GraphEdge<S>> {
        match root.as_ref() {
            &BDD::Choice(ref l, _, ref r) => {
                let _root = self.env.find(root);
                let _l = self.env.find(l);
                let _r = self.env.find(r);

                let l_edges = self.edges_recursive(l);
                let r_edges = self.edges_recursive(r);

                l_edges
                    .iter()
                    .chain(r_edges.iter())
                    .chain(&vec![
                        (_root.clone(), true, _l.clone()),
                        (_root.clone(), false, _r.clone()),
                    ])
                    .unique()
                    .cloned()
                    .collect()
            }
            &BDD::True | &BDD::False => vec![].into(),
        }
    }
}
