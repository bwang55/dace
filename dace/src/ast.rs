use std::fmt::{Debug, Formatter};
use std::ops::{Add, Div, Mul, Sub};
use std::rc::{Rc, Weak};

use crate::types;

/// Each loop and statement is a node in a loop tree.
#[derive(Debug)]
pub struct Node {
    pub stmt: Stmt,
    pub(crate) parent: Weak<Node>,
}

#[derive(Debug)]
pub enum Stmt {
    Loop(LoopStmt),
    Ref(AryRef),
    Block(Vec<Rc<Node>>),
    Branch(BranchStmt),
}

pub struct AryRef {
    pub name: String,
    /// array dimensions, e.g. [5,5]
    pub dim: Vec<usize>,
    pub indices: Vec<String>,
    /// Subscript expressions: one function for each data dimension.
    /// Each function takes the indices of its loop nest and returns indices of the array access.
    #[allow(clippy::type_complexity)]
    pub sub: Box<dyn for<'a> Fn(&'a [i32]) -> types::AryAcc>,
    pub base: Option<usize>,
    pub ref_id: Option<usize>,
    pub ri: Vec<String>,
}

pub struct BranchStmt {
    #[allow(clippy::type_complexity)]
    pub cond: Box<dyn Fn(&[i32]) -> bool>,
    pub then_body: Rc<Node>,
    pub else_body: Option<Rc<Node>>,
}

pub struct LoopStmt {
    pub iv: String,
    pub lb: LoopBound,
    pub ub: LoopBound,
    // The next two need the FnOnce trait, which we'll add later
    // Now we assume test is iv < ub
    pub test: Box<dyn Fn(i32, i32) -> bool>,
    // Now we assume step is iv = iv + 1
    pub step: Box<dyn Fn(i32) -> i32>,
    pub body: Vec<Rc<Node>>,
    pub rank: i32,
}

pub enum LoopBound {
    Fixed(i32),
    #[allow(clippy::type_complexity)]
    Dynamic(Box<types::DynamicBoundFunction>),
    Affine {
        a: Vec<i32>,
        b: i32,
    },
}

impl Debug for AryRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "ArrayRef({}, {:?}, base: {:?})",
            self.name,
            self.dim,
            self.base.unwrap_or(99999)
        )
    }
}

impl Debug for BranchStmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BranchStmt")
            .field("then", &self.then_body)
            .field("else", &self.else_body)
            .finish_non_exhaustive()
    }
}

impl Debug for LoopStmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoopStmt")
            .field("iv", &self.iv)
            .field("lb", &self.lb)
            .field("ub", &self.ub)
            // .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

impl Debug for LoopBound {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            LoopBound::Fixed(x) => write!(f, "Fixed({x})"),
            LoopBound::Dynamic(_) => write!(f, "Dynamic"),
            LoopBound::Affine { a, b } => write!(f, "Affine({:?}, {})", a, b),
        }
    }
}

impl From<i32> for LoopBound {
    fn from(value: i32) -> Self {
        LoopBound::Fixed(value)
    }
}

impl<F> From<F> for LoopBound
where
    for<'a> F: Fn(&'a [i32]) -> i32 + 'static,
{
    fn from(value: F) -> Self {
        LoopBound::Dynamic(Box::new(value))
    }
}

impl From<(Vec<i32>, i32)> for LoopBound {
    fn from(value: (Vec<i32>, i32)) -> Self {
        LoopBound::Affine {
            a: value.0,
            b: value.1,
        }
    }
}
//Example:
// impl Add for LoopBound {
//     type Output = Self;

//     fn add(self, other: Self) -> Self {
//         match (self, other) {
//             (LoopBound::Fixed(constant_a), LoopBound::Fixed(constant_b)) => LoopBound::Fixed(Add::add(constant_a, constant_b)),
//             (LoopBound::Fixed(constant_a), LoopBound::Dynamic(dynamic_b)) => LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
//                 Add::add(constant_a, dynamic_b(vector))
//             })),
//             (LoopBound::Dynamic(dynamic_a), LoopBound::Fixed(constant_b)) => LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
//                 Add::add(dynamic_a(vector), constant_b)
//             })),
//             (LoopBound::Dynamic(dynamic_a), LoopBound::Dynamic(dynamic_b)) => LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
//                 Add::add(dynamic_a(vector), dynamic_b(vector))
//             })),
//             _ => unimplemented!("The operation + is not implemented for some type Affine"),
//         }
//     }
// }
macro_rules! loopbound_binop {
    ($operation_trait:ident, $binop:ident) => {
        impl $operation_trait for LoopBound {
            type Output = Self;

            fn $binop(self, other: LoopBound) -> LoopBound {
                match (self, other) {
                    (LoopBound::Fixed(constant_a), LoopBound::Fixed(constant_b)) => {
                        LoopBound::Fixed($operation_trait::$binop(constant_a, constant_b))
                    }
                    (LoopBound::Fixed(constant_a), LoopBound::Dynamic(dynamic_b)) => {
                        LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
                            $operation_trait::$binop(constant_a, dynamic_b(vector))
                        }))
                    }
                    (LoopBound::Dynamic(dynamic_a), LoopBound::Fixed(constant_b)) => {
                        LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
                            $operation_trait::$binop(dynamic_a(vector), constant_b)
                        }))
                    }
                    (LoopBound::Dynamic(dynamic_a), LoopBound::Dynamic(dynamic_b)) => {
                        LoopBound::Dynamic(Box::new(move |vector: &[i32]| {
                            $operation_trait::$binop(dynamic_a(vector), dynamic_b(vector))
                        }))
                    }
                    _ => unimplemented!("The operation + is not implemented for some type Affine"),
                }
            }
        }
    };
}

loopbound_binop!(Add, add);
loopbound_binop!(Sub, sub);
loopbound_binop!(Mul, mul);
loopbound_binop!(Div, div);

#[macro_export]
macro_rules! dynamic {
    ($x:tt) => {
        $crate::ast::LoopBound::Dynamic($tt)
    };
}

#[macro_export]
macro_rules! loop_node {
    ($ivar:expr, $lb:expr => $ub:expr) => {
        $crate::ast::Node::new_loop($ivar, $lb.into(), $ub.into(), |i, ub| i < ub, |i| i + 1)
    };
    ($ivar:expr, $lb:expr => $ub:expr, step: $step:expr) => {
        $crate::ast::Node::new_loop($ivar, $lb.into(), $ub.into(), |i, ub| i < ub, $step)
    };
    ($ivar:expr, $lb:expr => $ub:expr, test: $test:expr) => {
        $crate::ast::Node::new_loop($ivar, $lb.into(), $ub.into(), $test, |i| i + 1)
    };
    ($ivar:expr, $lb:expr => $ub:expr, test: $test:expr, step: $step:expr) => {
        $crate::ast::Node::new_loop($ivar, $lb.into(), $ub.into(), $test, $step)
    };
}

#[macro_export]
macro_rules! branch_node {
    (if ($cond:expr) {$then:tt}) => {
        $crate::ast::Node::new_node($crate::ast::Stmt::Branch($crate::ast::BranchStmt {
            cond: Box::new($cond),
            then_body: $then,
            else_body: None,
        }))
    };
    (if ($cond:expr) {$then:tt} else {$else:tt}) => {
        $crate::ast::Node::new_node($crate::ast::Stmt::Branch($crate::ast::BranchStmt {
            cond: Box::new($cond),
            then_body: $then,
            else_body: Some($else),
        }))
    };
}

fn print_bounds(bound: &LoopBound) {
    match bound {
        LoopBound::Fixed(val) => print!("{}", val),
        LoopBound::Dynamic(_) => print!("Dynamic"),
        LoopBound::Affine { a, b } => print!("Affine({:?}, {})", a, b),
    }
}

impl Node {
    pub fn print_structure(&self, indent: usize) {
        let indentation = " ".repeat(indent);
        match &self.stmt {
            Stmt::Loop(loop_stmt) => {
                print!("{}Loop: {}: ", indentation, loop_stmt.iv);
                print_bounds(&loop_stmt.lb);
                print!(" to ");
                print_bounds(&loop_stmt.ub);
                // print!("  *Next index: {}", (loop_stmt.step)(0));
                println!(" *Rank_{}", loop_stmt.rank);

                for child in &loop_stmt.body {
                    child.print_structure(indent + 2);
                }
            }
            Stmt::Ref(ary_ref) => {
                if ary_ref.indices.is_empty() {
                    let indices = (ary_ref.sub)(&[0, 1, 2, 3]); // Assuming a 3-dimensional array
                    let named_indices: Vec<String> = indices
                        .iter()
                        .map(|val| match val {
                            0 => "i".to_string(),
                            1 => "j".to_string(),
                            2 => "k".to_string(),
                            3 => "l".to_string(),
                            _ => format!("Dimension > 3: {}", val),
                        })
                        .collect();
                    println!(
                        "{}{}[{}]",
                        indentation,
                        ary_ref.name,
                        named_indices.join(", ")
                    );
                } else {
                    println!(
                        "{}{}[{}]",
                        indentation,
                        ary_ref.name,
                        ary_ref.indices.join(", ")
                    );
                }
            }
            Stmt::Block(children) => {
                println!("{}Block", indentation);
                for child in children {
                    child.print_structure(indent + 2);
                }
            }
            Stmt::Branch(branch_stmt) => {
                println!("{}Branch then", indentation);
                branch_stmt.then_body.print_structure(indent + 2);
                if let Some(else_body) = &branch_stmt.else_body {
                    println!("{}Else", indentation);
                    else_body.print_structure(indent + 2);
                }
            }
        }
    }

    pub fn rank(&self) -> Option<i32> {
        self.loop_only(|lp| lp.rank)
    }

    /// Create a new Node with a given statement.
    pub fn new_node(a_stmt: Stmt) -> Rc<Node> {
        Rc::new(Node {
            stmt: a_stmt,
            parent: Weak::new(),
        })
    }

    pub fn new_ref<F>(ary_nm: &str, ary_dim: Vec<usize>, ary_sub: F) -> Rc<Node>
    where
        F: for<'a> Fn(&'a [i32]) -> types::AryAcc + 'static,
    {
        let ref_stmt = AryRef {
            name: ary_nm.to_string(),
            dim: ary_dim,
            indices: vec![],
            sub: Box::new(ary_sub),
            base: None,
            ref_id: None,
            ri: vec![],
        };
        Node::new_node(Stmt::Ref(ref_stmt))
    }

    pub fn new_loop<F, G>(ivar: &str, lb: LoopBound, ub: LoopBound, test: F, step: G) -> Rc<Self>
    where
        F: Fn(i32, i32) -> bool + 'static,
        G: Fn(i32) -> i32 + 'static,
    {
        let loop_stmt = LoopStmt {
            iv: ivar.to_string(),
            lb,
            ub,
            test: Box::new(test),
            step: Box::new(step),
            body: vec![],
            rank: 0,
        };
        Self::new_node(Stmt::Loop(loop_stmt))
    }

    pub fn new_single_loop(ivar: &str, low: i32, high: i32) -> Rc<Self> {
        Self::new_loop(
            ivar,
            LoopBound::Fixed(low),
            LoopBound::Fixed(high),
            |i, ub| i < ub,
            |i| i + 1,
        )
    }

    pub fn new_single_loop_dyn_ub(
        ivar: &str,
        low: i32,
        ub: Box<types::DynamicBoundFunction>,
    ) -> Rc<Self> {
        Self::new_loop(
            ivar,
            LoopBound::Fixed(low),
            LoopBound::Dynamic(ub),
            |i, ub| i < ub,
            |i| i + 1,
        )
    }

    /// Extend the body of a loop node with another node.
    pub fn extend_loop_body(lup: &mut Rc<Node>, stmt: &mut Rc<Node>) {
        let lup_node = unsafe { Rc::get_mut_unchecked(lup) };
        lup_node.loop_only_mut(|lp| lp.body.push(Rc::clone(stmt)));

        // officiating the parent-child relationship
        let stmt_node = unsafe { Rc::get_mut_unchecked(stmt) };
        stmt_node.parent = Rc::downgrade(lup);
    }

    pub fn loop_only<U, F>(&self, f: F) -> Option<U>
    where
        F: FnOnce(&LoopStmt) -> U,
    {
        match &self.stmt {
            Stmt::Loop(ref aloop) => Some(f(aloop)),
            _ => None,
        }
    }

    pub fn loop_only_mut<U, F>(&mut self, f: F) -> Option<U>
    where
        F: FnOnce(&mut LoopStmt) -> U,
    {
        match &mut self.stmt {
            Stmt::Loop(ref mut aloop) => Some(f(aloop)),
            _ => None,
        }
    }

    pub fn ref_only<U, F>(&self, f: F) -> Option<U>
    where
        F: FnOnce(&AryRef) -> U,
    {
        match &self.stmt {
            Stmt::Ref(ref a_ref) => Some(f(a_ref)),
            _ => None,
        }
    }

    pub fn ref_only_ref<'a, U, F>(&'a self, f: F) -> Option<&'a U>
    where
        F: FnOnce(&'a AryRef) -> &'a U,
    {
        match &self.stmt {
            Stmt::Ref(ref a_ref) => Some(f(a_ref)),
            _ => None,
        }
    }

    pub fn ref_only_mut_ref<'a, U, F>(&'a mut self, f: F) -> Option<&'a mut U>
    where
        F: FnOnce(&'a mut AryRef) -> &'a mut U,
    {
        match &mut self.stmt {
            Stmt::Ref(ref mut a_ref) => Some(f(a_ref)),
            _ => None,
        }
    }

    // pub fn loop_body<'a>(&'a self, i: usize) -> &'a Rc<Node> {
    // }

    pub fn get_lb(&self) -> Option<i32> {
        self.loop_only(|lp| {
            if let LoopBound::Fixed(lowerbound) = lp.lb {
                lowerbound
            } else {
                panic!("dynamic loop bound is not supported")
            }
        })
    }

    // Get the count of nodes in the loop tree.
    pub fn node_count(&self) -> u32 {
        match &self.stmt {
            //    The body of a loop is a vector of Node's, so we need to
            //    iterate over the vector and sum the sanity of each node.
            Stmt::Loop(a_loop) => 1 + a_loop.body.iter().map(|x| x.node_count()).sum::<u32>(),
            Stmt::Ref(_) => 1,
            Stmt::Block(children) => 1 + children.iter().map(|x| x.node_count()).sum::<u32>(),
            Stmt::Branch(stmt) => {
                stmt.then_body.node_count()
                    + stmt.else_body.as_ref().map_or(0, |x| x.node_count())
                    + 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::construct::loop_body;

    use super::*;

    #[test]
    fn acc_ref() {
        let ar = AryRef {
            name: "X".to_string(),
            dim: vec![10],
            indices: vec![],
            sub: Box::new(|iv| vec![(iv[0] as usize) + 1]),
            base: None,
            ref_id: None,
            ri: vec![],
        };
        assert_eq!((ar.sub)(&[1]), [2]);
    }

    #[test]
    fn matrix_ref() {
        let ar = AryRef {
            name: "A".to_string(),
            dim: vec![10, 10],
            indices: vec![],
            sub: Box::new(|ijk| vec![ijk[0] as usize, ijk[1] as usize]),
            base: None,
            ref_id: None,
            ri: vec![],
        };
        assert_eq!((ar.sub)(&[1, 2, 3]), [1, 2]);
    }

    #[test]
    fn matmul() {
        let n: usize = 100; // array dim
        let ubound = n as i32; // loop bound
                               // creating C[i,j] += A[i,k] * B[k,j]
        let ref_c = Node::new_ref("C", vec![n, n], |ijk| {
            vec![ijk[0] as usize, ijk[1] as usize]
        });
        let ref_a = Node::new_ref("A", vec![n, n], |ijk| {
            vec![ijk[0] as usize, ijk[2] as usize]
        });
        let ref_b = Node::new_ref("B", vec![n, n], |ijk| {
            vec![ijk[2] as usize, ijk[1] as usize]
        });
        // creating loop i = 0, n
        let mut i_loop = Node::new_single_loop("i", 0, ubound);
        // creating loop j = 0, n
        let mut j_loop = Node::new_single_loop("j", 0, ubound);
        // creating loop k = 0, n { s_ref }
        let mut k_loop = Node::new_single_loop("k", 0, ubound);
        [ref_c, ref_a, ref_b]
            .iter_mut()
            .for_each(|s| Node::extend_loop_body(&mut k_loop, s));

        loop_body(&[&mut j_loop, &mut i_loop, &mut k_loop]);

        assert_eq!(j_loop.node_count(), 6);
    }

    #[test]
    fn matmul_2() {
        //The original method
        let n: usize = 100; // array dim
        let ubound = n as i32; // loop bound
                               // creating C[i,j] += A[i,k] * B[k,j]
        let ref_c = Node::new_ref("C", vec![n, n], |ijk| {
            vec![ijk[0] as usize, ijk[1] as usize]
        });
        let ref_a = Node::new_ref("A", vec![n, n], |ijk| {
            vec![ijk[0] as usize, ijk[2] as usize]
        });
        let ref_b = Node::new_ref("B", vec![n, n], |ijk| {
            vec![ijk[2] as usize, ijk[1] as usize]
        });

        // creating loop k = 0, n { s_ref }
        let mut k_loop = Node::new_single_loop("k", 0, ubound);
        [ref_c, ref_a, ref_b]
            .iter_mut()
            .for_each(|s| Node::extend_loop_body(&mut k_loop, s));
        // creating loop j = 0, n
        let mut j_loop = Node::new_single_loop("j", 0, ubound);
        Node::extend_loop_body(&mut j_loop, &mut k_loop);
        // creating loop i = 0, n
        let mut i_loop = Node::new_single_loop("i", 0, ubound);
        Node::extend_loop_body(&mut i_loop, &mut j_loop);

        assert_eq!(i_loop.node_count(), 6);
    }

    #[test]
    fn example_dyn() {
        // for i in 0..n
        //     for j in 0 .. n - i
        let n: usize = 100; // array dim
        let ubound = n as i32; // loop bound
        let mut j_loop = Node::new_single_loop_dyn_ub("j", 0, Box::new(move |i| ubound - i[0]));
        // creating loop i = 0, n
        let mut i_loop = Node::new_single_loop("i", 0, ubound);
        Node::extend_loop_body(&mut i_loop, &mut j_loop);

        println!("{:?}", j_loop.stmt);
        assert_eq!(i_loop.node_count(), 2);
    }

    #[test]
    fn example_macro() {
        // for i in 0..n step by 2
        //     for j in 0 .. n - i
        let n: usize = 100; // array dim
        let ubound = n as i32; // loop bound
        let mut j_loop = loop_node!("j", 0 => move |i : &[i32]| ubound - i[0]);
        // creating loop i = 0, n
        let mut i_loop = loop_node!("i", 0 => ubound, step: |x| x + 2);
        Node::extend_loop_body(&mut i_loop, &mut j_loop);

        assert_eq!(i_loop.node_count(), 2);
    }
    #[test]
    fn example_if_then() {
        // for i in 0..n step by 2
        let ubound = 100; // loop bound

        let ref_a = Node::new_ref("A", vec![100], |i| vec![i[0] as usize]);

        let mut branch = branch_node! {
            if (|ivec| ivec[0] & 1 == 0) {
                ref_a
            }
        };

        // creating loop i = 0, n
        let mut i_loop = loop_node!("i", 0 => ubound, step: |x| x + 2);
        Node::extend_loop_body(&mut i_loop, &mut branch);

        assert_eq!(i_loop.node_count(), 3);
    }

    #[test]
    fn example_if_then_else() {
        // for i in 0..n step by 2
        let ubound = 100; // loop bound

        let ref_a = Node::new_ref("A", vec![100], |i| vec![i[0] as usize]);

        let ref_b = Node::new_ref("B", vec![100], |i| vec![i[0] as usize]);

        let mut branch = branch_node! {
            if (|ivec| ivec[0] & 1 == 0) {
                ref_a
            } else {
                ref_b
            }
        };

        // creating loop i = 0, n
        let mut i_loop = loop_node!("i", 0 => ubound, step: |x| x + 2);
        Node::extend_loop_body(&mut i_loop, &mut branch);

        assert_eq!(i_loop.node_count(), 4);
    }

    #[test]
    fn mat_transpose1() {
        let n: usize = 1024;
        let ubound = n as i32;
        // for (int c0 = 0; c0 < n; c0 += 1)
        // for (int c1 = 0; c1 < n; c1 += 1)
        //   x1[c0] = (x1[c0] + (A[c0][c1] * y_1[c1]));
        let ref_x1 = Node::new_ref("x1", vec![n], |ij| vec![ij[0] as usize]);
        let ref_a = Node::new_ref("a", vec![n, n], |ij| vec![ij[0] as usize, ij[1] as usize]);
        let ref_y1 = Node::new_ref("y1", vec![n], |ij| vec![ij[1] as usize]);

        let mut j_loop = Node::new_single_loop("j", 0, ubound);
        [ref_x1, ref_a, ref_y1]
            .iter_mut()
            .for_each(|s| Node::extend_loop_body(&mut j_loop, s));

        let mut i_loop = Node::new_single_loop("i", 0, ubound);
        Node::extend_loop_body(&mut i_loop, &mut j_loop);

        assert_eq!(i_loop.node_count(), 5);
    }

    // #[test]
    // fn mat_transpose2() {
    //     let n: usize = 1024;
    // 	let ubound = n as i32;
    //     //     for (int c0 = 0; c0 < n; c0 += 1)
    //     //     for (int c1 = 0; c1 < n; c1 += 1)
    //     //     x2[c0] = (x2[c0] + (A[c1][c0] * y_2[c1]));
    // }
}
