extern crate easter;

use easter::stmt::{Script, StmtListItem, Stmt};
use easter::decl::{Decl, Dtor};
use easter::expr::{ExprListItem, Expr};
use easter::patt::{Patt, AssignTarget};
use easter::obj::{Prop, PropVal};
use easter::fun::Fun;

/// An estree (easter crate) JavaScript AST walker.
pub struct Walker<'a, C: Callbacks> {
    ast: &'a Script,
    callbacks: C,
}

/// Holds functions to be called on different types of nodes.
/// There are functions for preorder traversal and postorder traversal.
/// All callbacks are optional, implementations can pick and choose which they need.
pub trait Callbacks {
    /// Called before a top-level Script node is entered.
    fn pre_script(&mut self, _node: &Script) -> () {}
    /// Called before a Statement node is entered.
    fn pre_stmt(&mut self, _node: &Stmt) -> () {}
    /// Called before an Expression node is entered.
    fn pre_expr(&mut self, _node: &Expr) -> () {}
    /// Called before a Declaration node is entered.
    fn pre_decl(&mut self, _node: &Decl) -> () {}
    /// Called before a Function node is entered.
    fn pre_fun<Id>(&mut self, _node: &Fun<Id>) -> () {}
    /// Called after a top-level Script node was handled.
    fn post_script(&mut self, _node: &Script) -> () {}
    /// Called after a Statement node was handled.
    fn post_stmt(&mut self, _node: &Stmt) -> () {}
    /// Called after an Expression node was handled.
    fn post_expr(&mut self, _node: &Expr) -> () {}
    /// Called after a Declaration node was handled.
    fn post_decl(&mut self, _node: &Decl) -> () {}
    /// Called after a Function node was handled.
    fn post_fun<Id>(&mut self, _node: &Fun<Id>) -> () {}
}

impl<'a, C: Callbacks> Walker<'a, C> {
    /// Create a new Walker for a given ESTree Script, calling the
    /// callbacks specified in `callbacks` on the relevant nodes.
    pub fn new(ast: &'a Script, callbacks: C) -> Walker<'a, C> {
        Walker { ast, callbacks }
    }

    /// Do a recursive walk, calling `callbacks` where relevant.
    /// Returns the Callbacks instance, so that custom implementations
    /// of this trait can contain state.
    /// Consumes the walker—create a new one to do more than one walk.
    pub fn walk(mut self) -> C {
        self.walk_script();
        self.callbacks
    }

    /// Kick off the walk at the top-level Script node.
    fn walk_script(&mut self) -> () {
        self.callbacks.pre_script(self.ast);
        for item in &self.ast.items {
            self.walk_stmt_item(item);
        }
        self.callbacks.post_script(self.ast);
    }

    /// Walk an item in a list of statements, like in { blocks; }.
    fn walk_stmt_item(&mut self, item: &StmtListItem) -> () {
        match *item {
            StmtListItem::Stmt(ref stmt) => self.walk_stmt(stmt),
            StmtListItem::Decl(ref decl) => self.walk_decl(decl),
        }
    }

    /// Walk a statement.
    fn walk_stmt(&mut self, stmt: &Stmt) -> () {
        self.callbacks.pre_stmt(stmt);
        match *stmt {
            Stmt::Block(ref block) => {
                for item in &block.items {
                    self.walk_stmt_item(item);
                }
            },
            Stmt::Var(_, ref decls, _) => self.walk_var(decls),
            Stmt::Expr(_, ref expr, _) => self.walk_expr(expr),
            Stmt::If(_, ref cond, ref cons, ref alt) => {
                self.walk_expr(cond);
                self.walk_stmt(cons.as_ref());
                if let Some(ref node) = *alt { self.walk_stmt(node.as_ref()); }
            },
            Stmt::Label(_, _, ref block) => self.walk_stmt(block.as_ref()),
            Stmt::Switch(_, ref cond, ref cases) => {
                self.walk_expr(cond);
                for case in cases {
                    if let Some(ref test) = case.test { self.walk_expr(test); }
                    for item in &case.body {
                        self.walk_stmt_item(item);
                    }
                }
            },
            Stmt::Return(_, Some(ref arg), _) | Stmt::Throw(_, ref arg, _) =>
                self.walk_expr(arg),
            Stmt::Try(_, ref block, ref caught, ref finally) => {
                for item in &block.items { self.walk_stmt_item(item); }
                if let Some(ref caught_block) = *caught {
                    for item in &caught_block.body.items { self.walk_stmt_item(item); }
                }
                if let Some(ref finally_block) = *finally {
                    for item in &finally_block.items { self.walk_stmt_item(item); }
                }
            },
            Stmt::While(_, ref cond, ref body) => {
                self.walk_expr(cond);
                self.walk_stmt(body.as_ref());
            },
            Stmt::DoWhile(_, ref body, ref cond, _) => {
                self.walk_stmt(body.as_ref());
                self.walk_expr(cond);
            },
            Stmt::For(_, ref _init, ref cond, ref update, ref body) => {
                // if let Some(ref node) = *head { self.walk_for_head(node); }
                if let Some(ref node) = *cond { self.walk_expr(node); }
                if let Some(ref node) = *update { self.walk_expr(node); }
                self.walk_stmt(body.as_ref());
            },
            Stmt::ForIn(_, ref _head, ref iterable, ref body) => {
                // if let Some(ref node) = *head { self.walk_for_in_head(node); }
                self.walk_expr(iterable);
                self.walk_stmt(body.as_ref());
            },
            Stmt::ForOf(_, ref _head, ref iterable, ref body) => {
                // if let Some(ref node) = *head { self.walk_for_of_head(node); }
                self.walk_expr(iterable);
                self.walk_stmt(body.as_ref());
            },
            _ => (),
        }
        self.callbacks.post_stmt(stmt);
    }

    /// Walk a declaration node (function, let, const).
    fn walk_decl(&mut self, decl: &Decl) -> () {
        self.callbacks.pre_decl(decl);
        match *decl {
            Decl::Fun(ref fun) => self.walk_fun(fun),
            Decl::Let(_, ref dtors, _) => {
                for dtor in dtors {
                    self.walk_dtor(dtor);
                }
            },
            Decl::Const(_, ref dtors, _) => {
                for dtor in dtors {
                    self.walk_patt(&dtor.patt);
                    self.walk_expr(&dtor.value);
                }
            },
        }
        self.callbacks.post_decl(decl);
    }

    /// Walk a var declaration.
    fn walk_var(&mut self, decls: &[Dtor]) -> () {
        for decl in decls {
            self.walk_dtor(decl);
        }
    }

    fn walk_dtor(&mut self, dtor: &Dtor) -> () {
        if let Dtor::Simple(_, _, Some(ref expr)) = *dtor {
            self.walk_expr(expr);
        }
    }

    /// Walk an expression node.
    fn walk_expr(&mut self, expr: &Expr) -> () {
        self.callbacks.pre_expr(expr);
        match *expr {
            // TODO move this into a callback
            // and move the walk_* functions to generic AST walker
            Expr::Call(_, ref callee, ref args) => {
                self.walk_expr(callee);
                for arg in args {
                    match *arg {
                        ExprListItem::Expr(ref node) => self.walk_expr(node),
                        ExprListItem::Spread(_, ref node) => self.walk_expr(node),
                    }
                }
            },
            Expr::Seq(_, ref exprs) => {
                for expr in exprs {
                    self.walk_expr(expr);
                }
            }
            Expr::Arr(_, ref elements) => {
                for el in elements {
                    match *el {
                        Some(ExprListItem::Expr(ref node)) => self.walk_expr(node),
                        Some(ExprListItem::Spread(_, ref node)) => self.walk_expr(node),
                        None => (),
                    }
                }
            },
            Expr::Obj(_, ref properties) => {
                for prop in properties {
                    self.walk_prop(prop);
                }
            },
            Expr::Fun(ref fun) => self.walk_fun(fun),
            Expr::Binop(_, _, ref a, ref b) | Expr::Logop(_, _, ref a, ref b) => {
                self.walk_expr(a.as_ref());
                self.walk_expr(b.as_ref());
            },
            Expr::Unop(_, _, ref expr) => self.walk_expr(expr.as_ref()),
            Expr::PreInc(_, ref target) | Expr::PostInc(_, ref target) |
            Expr::PreDec(_, ref target) | Expr::PostDec(_, ref target) =>
                self.walk_assign_target(target.as_ref()),
            Expr::Assign(_, ref target, ref expr) => {
                self.walk_patt(target);
                self.walk_expr(expr.as_ref());
            },
            Expr::BinAssign(_, _, ref target, ref expr) => {
                self.walk_assign_target(target);
                self.walk_expr(expr.as_ref());
            },
            Expr::Cond(_, ref cond, ref cons, ref alt) => {
                self.walk_expr(cond.as_ref());
                self.walk_expr(cons.as_ref());
                self.walk_expr(alt.as_ref());
            },
            // a.b
            Expr::Dot(_, ref object, ref _property) => self.walk_expr(object.as_ref()),
            Expr::Brack(_, ref object, ref property) => {
                self.walk_expr(object.as_ref());
                self.walk_expr(property.as_ref());
            },
            _ => (),
        }
        self.callbacks.post_expr(expr);
    }

    /// Walk a function declaration or expression node.
    fn walk_fun<Id>(&mut self, fun: &Fun<Id>) -> () {
        self.callbacks.pre_fun(fun);
        for item in &fun.body.items {
            self.walk_stmt_item(item);
        }
        self.callbacks.post_fun(fun);
    }

    fn walk_patt<T>(&mut self, _target: &Patt<T>) -> () {
        // ignore for now
    }

    fn walk_assign_target(&mut self, target: &AssignTarget) -> () {
        match *target {
            AssignTarget::Id(_) => (),
            AssignTarget::Dot(_, ref object, ref _property) => self.walk_expr(object.as_ref()),
            AssignTarget::Brack(_, ref object, ref property) => {
                self.walk_expr(object.as_ref());
                self.walk_expr(property.as_ref());
            },
        }
    }

    fn walk_prop(&mut self, prop: &Prop) -> () {
        match *prop {
            Prop::Regular(_, ref key, ref val) => {
                match *val {
                    PropVal::Init(ref value) => self.walk_expr(value),
                    PropVal::Get(_, ref body) | PropVal::Set(_, _, ref body) => {
                        for item in &body.items {
                            self.walk_stmt_item(item);
                        }
                    },
                }
            },
            Prop::Method(ref fun) => self.walk_fun(fun),
            Prop::Shorthand(ref id) => (),
        }
    }
}
