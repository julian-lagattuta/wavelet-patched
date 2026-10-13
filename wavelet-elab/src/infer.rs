//! Capability-independent type inference.
//!
//! Integer literals begin with unknown signedness. Context constrains them,
//! and every integer operation requires one exact signedness for all of its
//! operands and result. Wavelet has no implicit integer promotions or casts.

use std::collections::BTreeMap;

use crate::env::FnRegistry;
use crate::error::TypeError;
use crate::ir::{Expr, FnDef, Op, Program, Signedness, Stmt, Tail, Ty, TypedVar, UntypedVar, Val};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Kind {
    Int(usize),
    Exact(Ty),
}

#[derive(Clone, Debug)]
pub(crate) struct InferredTypes {
    types: BTreeMap<usize, Ty>,
}

impl InferredTypes {
    pub(crate) fn get(&self, var: &UntypedVar) -> Result<Ty, TypeError> {
        self.types
            .get(&(var as *const UntypedVar as usize))
            .cloned()
            .ok_or_else(|| TypeError::UndeclaredVar(var.0.clone()))
    }
}

struct InferCtx {
    scope: BTreeMap<String, Kind>,
    bindings: BTreeMap<usize, Kind>,
    signs: Vec<Option<Signedness>>,
    equalities: Vec<(usize, usize)>,
}

impl InferCtx {
    fn new(def: &FnDef<UntypedVar>) -> Result<Self, TypeError> {
        let mut ctx = Self {
            scope: BTreeMap::new(),
            bindings: BTreeMap::new(),
            signs: Vec::new(),
            equalities: Vec::new(),
        };
        for param in &def.params {
            ctx.bind_parameter(&param.name, &param.ty);
        }
        Ok(ctx)
    }

    fn mismatch(expected: &Ty, found: &Ty) -> TypeError {
        TypeError::TypeMismatch {
            expected: TypeError::type_name(expected),
            found: TypeError::type_name(found),
        }
    }

    fn bind_parameter(&mut self, name: &str, ty: &Ty) {
        let kind = match ty {
            Ty::Int(sign) => {
                let id = self.signs.len();
                self.signs.push(Some(*sign));
                Kind::Int(id)
            }
            _ => Kind::Exact(ty.clone()),
        };
        self.scope.insert(name.to_string(), kind);
    }

    fn declare_int(
        &mut self,
        var: &UntypedVar,
        sign: Option<Signedness>,
    ) -> Result<usize, TypeError> {
        let id = self.signs.len();
        self.signs.push(sign);
        let kind = Kind::Int(id);
        self.scope.insert(var.0.clone(), kind.clone());
        self.bindings
            .insert(var as *const UntypedVar as usize, kind);
        Ok(id)
    }

    fn declare_exact(&mut self, var: &UntypedVar, ty: &Ty) -> Result<Kind, TypeError> {
        if let Ty::Int(sign) = ty {
            return self.declare_int(var, Some(*sign)).map(Kind::Int);
        }
        let kind = Kind::Exact(ty.clone());
        self.scope.insert(var.0.clone(), kind.clone());
        self.bindings
            .insert(var as *const UntypedVar as usize, kind.clone());
        Ok(kind)
    }

    fn kind_of(&self, var: &UntypedVar) -> Result<Kind, TypeError> {
        self.scope
            .get(&var.0)
            .cloned()
            .ok_or_else(|| TypeError::UndeclaredVar(var.0.clone()))
    }

    fn int_of(&self, var: &UntypedVar, operation: &str) -> Result<usize, TypeError> {
        match self.kind_of(var)? {
            Kind::Int(id) => Ok(id),
            Kind::Exact(_) => Err(TypeError::InvalidOp {
                op: operation.to_string(),
            }),
        }
    }

    fn exact_of(&self, var: &UntypedVar) -> Result<Ty, TypeError> {
        match self.kind_of(var)? {
            Kind::Exact(ty) => Ok(ty),
            Kind::Int(id) => self.signs[id]
                .map(Ty::Int)
                .ok_or_else(|| TypeError::InvalidOp {
                    op: format!("type of '{}' is not yet resolved", var.0),
                }),
        }
    }

    fn pin(&mut self, id: usize, sign: Signedness) -> Result<(), TypeError> {
        match self.signs[id] {
            Some(existing) if existing != sign => {
                Err(Self::mismatch(&Ty::Int(existing), &Ty::Int(sign)))
            }
            Some(_) => Ok(()),
            None => {
                self.signs[id] = Some(sign);
                Ok(())
            }
        }
    }

    fn equal(&mut self, a: usize, b: usize) {
        self.equalities.push((a, b));
    }

    fn constrain_exact(&mut self, var: &UntypedVar, expected: &Ty) -> Result<(), TypeError> {
        match (self.kind_of(var)?, expected) {
            (Kind::Int(id), Ty::Int(sign)) => self.pin(id, *sign),
            (Kind::Exact(found), expected) if &found == expected => Ok(()),
            (Kind::Int(id), expected) => {
                let found = Ty::Int(self.signs[id].unwrap_or(Signedness::Unsigned));
                Err(Self::mismatch(expected, &found))
            }
            (Kind::Exact(found), expected) => Err(Self::mismatch(expected, &found)),
        }
    }

    fn collect_expr(
        &mut self,
        expr: &Expr<UntypedVar>,
        expected: &Ty,
        registry: &FnRegistry,
    ) -> Result<(), TypeError> {
        for stmt in &expr.stmts {
            self.collect_stmt(stmt, registry)?;
        }
        self.collect_tail(&expr.tail, expected, registry)
    }

    fn collect_stmt(
        &mut self,
        stmt: &Stmt<UntypedVar>,
        registry: &FnRegistry,
    ) -> Result<(), TypeError> {
        match stmt {
            Stmt::LetVal { var, val, .. } => match val {
                Val::Int(n) => {
                    self.declare_int(
                        var,
                        if *n < 0 {
                            Some(Signedness::Signed)
                        } else {
                            None
                        },
                    )?;
                    Ok(())
                }
                Val::Bool(_) => self.declare_exact(var, &Ty::Bool).map(|_| ()),
                Val::Unit => self.declare_exact(var, &Ty::Unit).map(|_| ()),
            },
            Stmt::LetOp { vars, op, .. } => self.collect_op(vars, op),
            Stmt::LetCall {
                vars, func, args, ..
            } => {
                let callee = registry
                    .get(func)
                    .ok_or_else(|| TypeError::UndefinedFunction(func.0.clone()))?;
                self.collect_call_args(callee, args, "function call")?;
                if vars.len() != 1 {
                    return Err(TypeError::InvalidOp {
                        op: format!("function call to {} must bind one result", func.0),
                    });
                }
                self.declare_exact(&vars[0], &callee.returns)?;
                Ok(())
            }
        }
    }

    fn collect_op(&mut self, vars: &[UntypedVar], op: &Op<UntypedVar>) -> Result<(), TypeError> {
        let opname = format!("{op:?}");
        match op {
            Op::Add
            | Op::Sub
            | Op::Mul
            | Op::Sdiv
            | Op::Udiv
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor => {
                if vars.len() != 3 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                let lhs = self.int_of(&vars[0], &opname)?;
                let rhs = self.int_of(&vars[1], &opname)?;
                let result = self.declare_int(&vars[2], None)?;
                self.equal(lhs, rhs);
                self.equal(lhs, result);
                if matches!(op, Op::Udiv) {
                    self.pin(lhs, Signedness::Unsigned)?;
                }
                Ok(())
            }
            Op::Shl | Op::Ashr | Op::Lshr => {
                if vars.len() != 3 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                let value = self.int_of(&vars[0], &opname)?;
                // Shift counts are independently typed integers; only the
                // shifted value determines the result type.
                self.int_of(&vars[1], &opname)?;
                let result = self.declare_int(&vars[2], None)?;
                self.equal(value, result);
                if matches!(op, Op::Lshr) {
                    self.pin(value, Signedness::Unsigned)?;
                }
                Ok(())
            }
            Op::And | Op::Or => {
                if vars.len() != 3 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                self.constrain_exact(&vars[0], &Ty::Bool)?;
                self.constrain_exact(&vars[1], &Ty::Bool)?;
                self.declare_exact(&vars[2], &Ty::Bool)?;
                Ok(())
            }
            Op::Not => {
                if vars.len() != 2 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                self.constrain_exact(&vars[0], &Ty::Bool)?;
                self.declare_exact(&vars[1], &Ty::Bool)?;
                Ok(())
            }
            Op::SignedLessThan
            | Op::SignedLessEqual
            | Op::UnsignedLessThan
            | Op::UnsignedLessEqual => {
                if vars.len() != 3 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                let lhs = self.int_of(&vars[0], &opname)?;
                let rhs = self.int_of(&vars[1], &opname)?;
                self.equal(lhs, rhs);
                if matches!(op, Op::UnsignedLessThan | Op::UnsignedLessEqual) {
                    self.pin(lhs, Signedness::Unsigned)?;
                }
                self.declare_exact(&vars[2], &Ty::Bool)?;
                Ok(())
            }
            Op::Equal | Op::NotEqual => {
                if vars.len() != 3 {
                    return Err(TypeError::InvalidOp { op: opname });
                }
                match (self.kind_of(&vars[0])?, self.kind_of(&vars[1])?) {
                    (Kind::Int(lhs), Kind::Int(rhs)) => self.equal(lhs, rhs),
                    (Kind::Exact(lhs), Kind::Exact(rhs)) if lhs == rhs => {}
                    _ => return Err(TypeError::InvalidOp { op: opname }),
                }
                self.declare_exact(&vars[2], &Ty::Bool)?;
                Ok(())
            }
            Op::Load { array, index, len } => {
                if vars.len() != 1 {
                    return Err(TypeError::InvalidOp {
                        op: "load destination arity".into(),
                    });
                }
                self.int_of(index, "load index type")?;
                let arr_ty = self.exact_of(array)?;
                let (elem, arr_len) = match arr_ty {
                    Ty::RefShrd { elem, len } | Ty::RefUniq { elem, len } => (elem, len),
                    _ => {
                        return Err(TypeError::InvalidOp {
                            op: "load non array".into(),
                        })
                    }
                };
                if arr_len != *len {
                    return Err(TypeError::InvalidOp {
                        op: "load length mismatch".into(),
                    });
                }
                self.declare_exact(&vars[0], &elem)?;
                Ok(())
            }
            Op::Store {
                array,
                index,
                value,
                len,
            } => {
                if !vars.is_empty() {
                    return Err(TypeError::InvalidOp {
                        op: "store destination arity".into(),
                    });
                }
                self.int_of(index, "store index type")?;
                let arr_ty = self.exact_of(array)?;
                let (elem, arr_len) = match arr_ty {
                    Ty::RefShrd { elem, len } | Ty::RefUniq { elem, len } => (elem, len),
                    _ => {
                        return Err(TypeError::InvalidOp {
                            op: "store non array".into(),
                        })
                    }
                };
                if arr_len != *len {
                    return Err(TypeError::InvalidOp {
                        op: "store length mismatch".into(),
                    });
                }
                self.constrain_exact(value, &elem)
            }
        }
    }

    fn collect_tail(
        &mut self,
        tail: &Tail<UntypedVar>,
        expected: &Ty,
        registry: &FnRegistry,
    ) -> Result<(), TypeError> {
        match tail {
            Tail::RetVar(var) => self.constrain_exact(var, expected),
            Tail::IfElse {
                cond,
                then_e,
                else_e,
            } => {
                self.constrain_exact(cond, &Ty::Bool)?;
                let parent = self.scope.clone();
                self.collect_expr(then_e, expected, registry)?;
                self.scope = parent.clone();
                self.collect_expr(else_e, expected, registry)?;
                self.scope = parent;
                Ok(())
            }
            Tail::TailCall { func, args } => {
                let callee = registry
                    .get(func)
                    .ok_or_else(|| TypeError::UndefinedFunction(func.0.clone()))?;
                if &callee.returns != expected {
                    return Err(Self::mismatch(expected, &callee.returns));
                }
                self.collect_call_args(callee, args, "tail call")
            }
        }
    }

    fn collect_call_args(
        &mut self,
        callee: &FnDef<UntypedVar>,
        args: &[UntypedVar],
        call_kind: &str,
    ) -> Result<(), TypeError> {
        if args.len() != callee.params.len() {
            return Err(TypeError::InvalidOp {
                op: format!(
                    "{call_kind} to {} has wrong number of arguments",
                    callee.name.0
                ),
            });
        }
        for (param, arg) in callee.params.iter().zip(args) {
            self.constrain_exact(arg, &param.ty)?;
        }
        Ok(())
    }

    fn solve(self) -> Result<InferredTypes, TypeError> {
        let mut parent: Vec<usize> = (0..self.signs.len()).collect();
        for (a, b) in &self.equalities {
            union(&mut parent, *a, *b);
        }
        let mut component_signs = BTreeMap::new();
        for (id, sign) in self.signs.iter().enumerate() {
            let Some(sign) = sign else { continue };
            let root = find(&mut parent, id);
            if let Some(existing) = component_signs.insert(root, *sign) {
                if existing != *sign {
                    return Err(Self::mismatch(&Ty::Int(existing), &Ty::Int(*sign)));
                }
            }
        }
        let mut types = BTreeMap::new();
        for (binding, kind) in self.bindings {
            let ty = match kind {
                Kind::Exact(ty) => ty,
                Kind::Int(id) => {
                    let root = find(&mut parent, id);
                    Ty::Int(*component_signs.get(&root).unwrap_or(&Signedness::Unsigned))
                }
            };
            types.insert(binding, ty);
        }
        Ok(InferredTypes { types })
    }
}

fn find(parent: &mut [usize], id: usize) -> usize {
    if parent[id] != id {
        parent[id] = find(parent, parent[id]);
    }
    parent[id]
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let a = find(parent, a);
    let b = find(parent, b);
    if a != b {
        parent[b] = a;
    }
}

pub(crate) fn infer_fn_types(
    def: &FnDef<UntypedVar>,
    registry: &FnRegistry,
) -> Result<InferredTypes, TypeError> {
    let mut ctx = InferCtx::new(def)?;
    ctx.collect_expr(&def.body, &def.returns, registry)?;
    ctx.solve()
}

/// Infer all binding types without performing capability checking.
pub fn infer_program(program: &Program<UntypedVar>) -> Result<Program<TypedVar>, TypeError> {
    let mut registry = FnRegistry::default();
    for definition in &program.defs {
        registry.insert(definition.clone());
    }

    let defs = program
        .defs
        .iter()
        .map(|definition| infer_function(definition, &registry))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Program { defs })
}

fn infer_function(
    definition: &FnDef<UntypedVar>,
    registry: &FnRegistry,
) -> Result<FnDef<TypedVar>, TypeError> {
    let inferred = infer_fn_types(definition, registry)?;
    let mut scope = definition
        .params
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.ty.clone()))
        .collect();
    let body = annotate_expr(&definition.body, &inferred, &mut scope)?;
    Ok(FnDef {
        name: definition.name.clone(),
        params: definition.params.clone(),
        alloc_arrays: definition.alloc_arrays.clone(),
        caps: definition.caps.clone(),
        returns: definition.returns.clone(),
        body,
    })
}

type TypeScope = BTreeMap<String, Ty>;

fn use_var(var: &UntypedVar, scope: &TypeScope) -> Result<TypedVar, TypeError> {
    scope
        .get(&var.0)
        .cloned()
        .map(|ty| var.add_type(ty))
        .ok_or_else(|| TypeError::UndeclaredVar(var.0.clone()))
}

fn bind_var(
    var: &UntypedVar,
    inferred: &InferredTypes,
    scope: &mut TypeScope,
) -> Result<TypedVar, TypeError> {
    let ty = inferred.get(var)?;
    scope.insert(var.0.clone(), ty.clone());
    Ok(var.add_type(ty))
}

fn annotate_expr(
    expression: &Expr<UntypedVar>,
    inferred: &InferredTypes,
    scope: &mut TypeScope,
) -> Result<Expr<TypedVar>, TypeError> {
    let stmts = expression
        .stmts
        .iter()
        .map(|statement| annotate_stmt(statement, inferred, scope))
        .collect::<Result<Vec<_>, _>>()?;
    let tail = annotate_tail(&expression.tail, inferred, scope)?;
    Ok(Expr { stmts, tail })
}

fn annotate_stmt(
    statement: &Stmt<UntypedVar>,
    inferred: &InferredTypes,
    scope: &mut TypeScope,
) -> Result<Stmt<TypedVar>, TypeError> {
    match statement {
        Stmt::LetVal { var, val, fence } => Ok(Stmt::LetVal {
            var: bind_var(var, inferred, scope)?,
            val: val.clone(),
            fence: *fence,
        }),
        Stmt::LetCall {
            vars,
            func,
            args,
            fence,
        } => {
            let typed_args = args
                .iter()
                .map(|argument| use_var(argument, scope))
                .collect::<Result<Vec<_>, _>>()?;
            let typed_vars = vars
                .iter()
                .map(|var| bind_var(var, inferred, scope))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Stmt::LetCall {
                vars: typed_vars,
                func: func.clone(),
                args: typed_args,
                fence: *fence,
            })
        }
        Stmt::LetOp { vars, op, fence } => {
            let input_count = match op {
                Op::Not => 1,
                Op::Load { .. } => 0,
                Op::Store { .. } => 0,
                _ => 2,
            };
            let mut typed_vars = vars[..input_count]
                .iter()
                .map(|var| use_var(var, scope))
                .collect::<Result<Vec<_>, _>>()?;
            for var in &vars[input_count..] {
                typed_vars.push(bind_var(var, inferred, scope)?);
            }
            let typed_op = annotate_op(op, &typed_vars, scope)?;
            Ok(Stmt::LetOp {
                vars: typed_vars,
                op: typed_op,
                fence: *fence,
            })
        }
    }
}

fn annotate_op(
    operation: &Op<UntypedVar>,
    vars: &[TypedVar],
    scope: &TypeScope,
) -> Result<Op<TypedVar>, TypeError> {
    let result_sign = || vars.last().and_then(|var| var.ty.signedness());
    Ok(match operation {
        Op::Add => Op::Add,
        Op::Sub => Op::Sub,
        Op::Mul => Op::Mul,
        Op::Sdiv if result_sign() == Some(Signedness::Unsigned) => Op::Udiv,
        Op::Sdiv => Op::Sdiv,
        Op::Udiv => Op::Udiv,
        Op::And => Op::And,
        Op::Or => Op::Or,
        Op::Not => Op::Not,
        Op::BitAnd => Op::BitAnd,
        Op::BitOr => Op::BitOr,
        Op::BitXor => Op::BitXor,
        Op::Shl => Op::Shl,
        Op::Ashr if result_sign() == Some(Signedness::Unsigned) => Op::Lshr,
        Op::Ashr => Op::Ashr,
        Op::Lshr => Op::Lshr,
        Op::SignedLessThan if vars[0].ty == Ty::Int(Signedness::Unsigned) => Op::UnsignedLessThan,
        Op::SignedLessThan => Op::SignedLessThan,
        Op::SignedLessEqual if vars[0].ty == Ty::Int(Signedness::Unsigned) => Op::UnsignedLessEqual,
        Op::SignedLessEqual => Op::SignedLessEqual,
        Op::UnsignedLessThan => Op::UnsignedLessThan,
        Op::UnsignedLessEqual => Op::UnsignedLessEqual,
        Op::Equal => Op::Equal,
        Op::NotEqual => Op::NotEqual,
        Op::Load { array, index, len } => Op::Load {
            array: use_var(array, scope)?,
            index: use_var(index, scope)?,
            len: len.clone(),
        },
        Op::Store {
            array,
            index,
            value,
            len,
        } => Op::Store {
            array: use_var(array, scope)?,
            index: use_var(index, scope)?,
            value: use_var(value, scope)?,
            len: len.clone(),
        },
    })
}

fn annotate_tail(
    tail: &Tail<UntypedVar>,
    inferred: &InferredTypes,
    scope: &TypeScope,
) -> Result<Tail<TypedVar>, TypeError> {
    Ok(match tail {
        Tail::RetVar(var) => Tail::RetVar(use_var(var, scope)?),
        Tail::TailCall { func, args } => Tail::TailCall {
            func: func.clone(),
            args: args
                .iter()
                .map(|argument| use_var(argument, scope))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Tail::IfElse {
            cond,
            then_e,
            else_e,
        } => {
            let mut then_scope = scope.clone();
            let mut else_scope = scope.clone();
            Tail::IfElse {
                cond: use_var(cond, scope)?,
                then_e: Box::new(annotate_expr(then_e, inferred, &mut then_scope)?),
                else_e: Box::new(annotate_expr(else_e, inferred, &mut else_scope)?),
            }
        }
    })
}
