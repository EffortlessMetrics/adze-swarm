use super::*;
use crate::pure_parser::Point;

fn create_test_node() -> Node {
    Node {
        symbol: 1,
        children: vec![],
        start_byte: 0,
        end_byte: 10,
        start_point: Point { row: 0, column: 0 },
        end_point: Point { row: 0, column: 10 },
        is_extra: false,
        is_error: false,
        is_missing: false,
        is_named: true,
        field_id: None,
        language: None,
    }
}

#[test]
fn test_stats_visitor() {
    let mut visitor = StatsVisitor::default();
    let node = create_test_node();
    visitor.enter_node(&node);
    visitor.visit_leaf(&node, "test");
    visitor.leave_node(&node);
    assert_eq!(visitor.total_nodes, 1);
    assert_eq!(visitor.leaf_nodes, 1);
    assert_eq!(visitor.max_depth, 1);
}

#[derive(Default)]
#[allow(dead_code)]
struct TestVisitor {
    entered_nodes: Vec<String>,
    left_nodes: Vec<String>,
    leaves: Vec<String>,
    errors: Vec<String>,
}

impl Visitor for TestVisitor {
    fn enter_node(&mut self, _node: &Node) -> VisitorAction {
        self.entered_nodes.push("node".to_string());
        VisitorAction::Continue
    }

    fn leave_node(&mut self, _node: &Node) {
        self.left_nodes.push("node".to_string());
    }

    fn visit_leaf(&mut self, _node: &Node, text: &str) {
        self.leaves.push(text.to_string());
    }

    fn visit_error(&mut self, _node: &Node) {
        self.errors.push("error".to_string());
    }
}

#[test]
fn test_pretty_print_visitor() {
    let mut visitor = PrettyPrintVisitor::new();
    let node = create_test_node();
    visitor.enter_node(&node);
    visitor.visit_leaf(&node, "hello");
    visitor.leave_node(&node);

    let output = visitor.output();
    assert!(output.contains("hello"));
}

#[test]
fn test_visitor_action() {
    assert_eq!(VisitorAction::Continue, VisitorAction::Continue);
    assert_ne!(VisitorAction::Continue, VisitorAction::Stop);
    assert_ne!(VisitorAction::SkipChildren, VisitorAction::Stop);
}

#[test]
fn test_tree_walker_creation() {
    let source = b"test source";
    let walker = TreeWalker::new(source);
    assert_eq!(walker.source, source);
}

#[test]
fn test_stop_visitor() {
    struct StopVisitor {
        count: usize,
    }

    impl Visitor for StopVisitor {
        fn enter_node(&mut self, _node: &Node) -> VisitorAction {
            self.count += 1;
            if self.count > 2 {
                VisitorAction::Stop
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut visitor = StopVisitor { count: 0 };
    let node = create_test_node();
    let _ = visitor.enter_node(&node);
    let _ = visitor.enter_node(&node);
    let action = visitor.enter_node(&node);
    assert_eq!(action, VisitorAction::Stop);
}

#[test]
fn test_skip_children_visitor() {
    struct SkipVisitor {
        depth: usize,
    }

    impl Visitor for SkipVisitor {
        fn enter_node(&mut self, _node: &Node) -> VisitorAction {
            self.depth += 1;
            if self.depth > 1 {
                VisitorAction::SkipChildren
            } else {
                VisitorAction::Continue
            }
        }
    }

    let mut visitor = SkipVisitor { depth: 0 };
    let node = create_test_node();
    assert_eq!(visitor.enter_node(&node), VisitorAction::Continue);
    assert_eq!(visitor.enter_node(&node), VisitorAction::SkipChildren);
}
