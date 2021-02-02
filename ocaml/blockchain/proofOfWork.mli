open Proposal;;

module type ProofOfWork = sig
  (* the type of the block's implementation *)
  type block
  (* the type of the hash's implementation *)
  type hash
  (* the type of the nonce *)
  type nonce
  (* the type of the acceptance threshold (should be equal to the type of hash) *)
  type threshold


  (* the initial nonce that should be used *)
  val init_nonce : nonce
  (* receives a nonce and returns the next one to be used *)
  val next_nonce : nonce ->nonce
  (* computes a hash, given a block and a nonce *)
  val compute_hash : block ->nonce ->hash
  (* accepts (true) or refuses (false) a hash by comparing it to a threshold *)
  val acceptance_function : hash ->threshold ->bool

end
