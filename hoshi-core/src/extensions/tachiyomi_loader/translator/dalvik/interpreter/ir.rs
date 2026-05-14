#[derive(Debug, Clone, PartialEq)]
pub enum JsExpr {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Reg(u8),
    This,
    MethodCall { receiver: Box<JsExpr>, method: String, args: Vec<JsExpr> },
    StaticCall  { class: String, method: String, args: Vec<JsExpr> },
    New         { class: String, args: Vec<JsExpr> },
    FieldGet    { receiver: Box<JsExpr>, field: String },
    BinOp       { op: &'static str, left: Box<JsExpr>, right: Box<JsExpr> },
    UnaryOp     { op: &'static str, expr: Box<JsExpr> },
    BitMask     { expr: Box<JsExpr>, mask: &'static str },
    Index       { arr: Box<JsExpr>, idx: Box<JsExpr> },
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsStmt {
    Assign   { reg: u8, expr: JsExpr },
    FieldSet { receiver: JsExpr, field: String, value: JsExpr },
    ArraySet { arr: JsExpr, idx: JsExpr, value: JsExpr },
    Expr(JsExpr),
    Return(Option<JsExpr>),
    If     { cond: JsExpr, then_body: Vec<JsStmt>, else_body: Vec<JsStmt> },
    Loop   { body: Vec<JsStmt> },
    Switch { expr: JsExpr, cases: Vec<(i32, Vec<JsStmt>)> },
    While {
        cond: JsExpr,
        body: Vec<JsStmt>,
    },
    DoWhile {
        body: Vec<JsStmt>,
        cond: JsExpr,
    },
    Break,
    Continue,
    Comment(String),
    CondGoto { cond: JsExpr, target: i32 },
    Goto(i32),
    Throw,
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Return(Option<JsExpr>),
    Throw,
    Goto(i32),
    CondGoto { cond: JsExpr, if_true: i32, if_false: i32 },
    Switch   { expr: JsExpr, cases: Vec<(i32, i32)>, default: i32 },
    FallThrough(i32),
    ImplicitReturn,
}

#[derive(Debug)]
pub struct BasicBlock {
    pub offset: i32,
    pub stmts:  Vec<JsStmt>,
    pub term:   Terminator,
}

#[derive(Debug, Clone)]
pub struct TaggedStmt {
    pub offset: i32,
    pub stmt:   JsStmt,
}

pub fn negate(cond: JsExpr) -> JsExpr {
    match cond {
        JsExpr::BinOp { op, left, right } => {
            let flipped = match op {
                "==" => "!=",  "!=" => "==",
                "===" => "!==", "!==" => "===",
                "<"  => ">=",  ">=" => "<",
                ">"  => "<=",  "<=" => ">",
                _ => return JsExpr::UnaryOp {
                    op: "!",
                    expr: Box::new(JsExpr::BinOp { op, left, right }),
                },
            };
            JsExpr::BinOp { op: flipped, left, right }
        }
        JsExpr::UnaryOp { op: "!", expr } if matches!(*expr, JsExpr::UnaryOp { op: "!", .. }) => {
            if let JsExpr::UnaryOp { expr: inner, .. } = *expr { *inner } else { unreachable!() }
        }
        other => JsExpr::UnaryOp { op: "!", expr: Box::new(other) },
    }
}