module type Block = sig
  (* the type of a block's hash *)
  type hash
  (* type representation of a block *)
  type t

  (* compute the hash of a block *)
  val compute_hash : t -> hash
  (* validate a block and it's contents *)
  val validate : t ->bool
end
