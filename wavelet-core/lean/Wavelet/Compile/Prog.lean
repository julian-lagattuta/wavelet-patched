import Wavelet.Seq.Prog

import Wavelet.Compile.Fn
import Wavelet.Compile.MapChans

/-! Compiler from `Prog` to dataflow `Proc`. -/

namespace Wavelet.Compile

open Semantics Seq Dataflow

/-- Channel name prefixes to disambiguate names during linking. -/
inductive LinkName (χ : Type u) where
  | base (name : χ)
  | main (name : LinkName χ)
  | dep (op : Nat) (name : LinkName χ)
  deriving Repr, DecidableEq, Hashable, Lean.ToJson, Lean.FromJson

def linkAtomicProc
  [Arity Op] [NeZeroArity Op]
  {sigs : Sigs k} [NeZeroSigs sigs]
  (k' : Fin (k + 1))
  (procs : (i : Fin k') → Proc Op (LinkName χ) V (sigs ↓i).ι (sigs ↓i).ω)
  (atom : AtomicProc (Op ⊕ (SigOps sigs k')) (LinkName χ) V)
  : AtomicProcs Op (LinkName χ) V :=
  match atom with
  | .op (.inl o) inputs outputs =>
    [.op o (inputs.map .main) (outputs.map .main)]
  | .op (.inr op) inputs outputs =>
    [.forward (inputs.map .main) ((procs op.toFin').inputs.map (LinkName.dep op.toFin))] ++
    (procs op.toFin').atoms.mapChans (LinkName.dep op.toFin) ++
    [.forward ((procs op.toFin').outputs.map (LinkName.dep op.toFin)) (outputs.map .main)]
  | .async aop inputs outputs =>
    [.async aop (inputs.map .main) (outputs.map .main)]

/-- Function operators occurring at the static call sites in a process. -/
def callTargets
  [Arity Op]
  {sigs : Sigs k} {k' : Fin (k + 1)}
  (atoms : AtomicProcs (Op ⊕ SigOps sigs k') χ V) : List (SigOps sigs k') :=
  atoms.filterMap fun
    | .op (.inr op) _ _ => some op
    | _ => none

def linkProcs
  [Arity Op] [NeZeroArity Op]
  {sigs : Sigs k} [NeZeroSigs sigs]
  (k' : Fin (k + 1))
  (procs : (i : Fin k') → Proc Op (LinkName χ) V (sigs ↓i).ι (sigs ↓i).ω)
  (main : Proc (Op ⊕ SigOps sigs k') (LinkName χ) V m n)
  : Proc Op (LinkName χ) V m n := {
    inputs := main.inputs.map LinkName.main,
    outputs := main.outputs.map LinkName.main,
    atoms := if (callTargets main.atoms).Nodup then
      (main.atoms.map (linkAtomicProc k' procs)).flatten
    else
      -- Keep the existing namespaces when every callee has one call site.
      -- Repeated calls need a separate copy of the callee's channels per site.
      (main.atoms.mapIdx fun site atom =>
        linkAtomicProc k'
          (fun i => (procs i).mapChans (LinkName.dep site)) atom).flatten,
  }

/-- Given a program (a list of functions that non-recursively call each other),
compile the `i`-th function to a process without any dependencies. -/
def compileProg
  [Arity Op] [DecidableEq χ] [InterpConsts V] [NeZeroArity Op]
  {sigs : Sigs k} [NeZeroSigs sigs]
  (prog : Prog Op χ V sigs)
  (i : Fin k) : Proc Op (LinkName (ChanName χ)) V (sigs i).ι (sigs i).ω :=
  -- Compile the current function
  let proc : Proc (Op ⊕ SigOps sigs i.castSucc) (LinkName (ChanName χ)) V _ _ :=
    compileFn (prog i) |>.mapChans LinkName.base
  -- Compile dependencies
  let deps : (j : Fin i) →
    Proc Op (LinkName (ChanName χ)) V
      (sigs (j.castLT (by omega))).ι
      (sigs (j.castLT (by omega))).ω :=
    λ j => compileProg prog (j.castLT (by omega))
  -- Link everything into one dataflow graph
  linkProcs i.castSucc deps proc

end Wavelet.Compile
