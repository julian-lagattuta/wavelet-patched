import Wavelet.Thm.Compile.AffineOp
import Wavelet.Thm.Data.List

namespace Wavelet.Compile
open Semantics Seq Dataflow

private theorem callTargets_nodup_aux
  [Arity Op]
  {sigs : Sigs k} {k' : Fin (k + 1)}
  (atoms : AtomicProcs (Op ⊕ SigOps sigs k') χ V)
  (haff : ∀ {i j : Nat} {op inputs outputs op' inputs' outputs'},
    i ≠ j → (_ : i < atoms.length) → (_ : j < atoms.length) →
    atoms[i] = .op (.inr op) inputs outputs →
    atoms[j] = .op (.inr op') inputs' outputs' → op ≠ op') :
  (callTargets atoms).Nodup := by
  induction atoms with
  | nil => simp [callTargets]
  | cons atom atoms ih =>
    have htail : ∀ {i j : Nat} {op inputs outputs op' inputs' outputs'},
        i ≠ j → (_ : i < atoms.length) → (_ : j < atoms.length) →
        atoms[i] = .op (.inr op) inputs outputs →
        atoms[j] = .op (.inr op') inputs' outputs' → op ≠ op' := by
      intro i j op inputs outputs op' inputs' outputs' hne hi hj hgeti hgetj
      exact haff (i := i + 1) (j := j + 1) (by omega)
        (by simp; omega) (by simp; omega)
        (by simpa using hgeti) (by simpa using hgetj)
    cases atom with
    | async aop inputs outputs => simpa [callTargets] using ih htail
    | op op inputs outputs =>
      cases op with
      | inl op => simpa [callTargets] using ih htail
      | inr op =>
        change (op :: callTargets atoms).Nodup
        rw [List.nodup_cons]
        refine ⟨?_, ih htail⟩
        intro hmem
        obtain ⟨atom, hmem, hget⟩ := List.mem_filterMap.mp hmem
        cases atom with
        | async aop inputs' outputs' => simp at hget
        | op op' inputs' outputs' =>
          cases op' with
          | inl op' => simp at hget
          | inr op' =>
            simp at hget
            subst op'
            obtain ⟨j, hj, hgetj⟩ := List.getElem_of_mem hmem
            exact haff (i := 0) (j := j + 1) (by omega)
              (by simp) (by simp; omega) rfl (by simpa using hgetj) rfl

@[simp] theorem callTargets_nodup_of_affine
  [Arity Op]
  {sigs : Sigs k} {k' : Fin (k + 1)}
  {main : Proc (Op ⊕ SigOps sigs k') χ V m n}
  (haff : main.AffineInrOp) : (callTargets main.atoms).Nodup :=
  callTargets_nodup_aux main.atoms haff

theorem dependency_instances_disjoint
  {χ : Type u} {op i j : Nat} {a b : LinkName χ} (hne : i ≠ j) :
  LinkName.dep op (.dep i a) ≠ .dep op (.dep j b) := by
  simp_all

end Wavelet.Compile
