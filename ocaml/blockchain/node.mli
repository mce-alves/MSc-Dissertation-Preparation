module type Node = sig
  (* type of the blocktree *)
  type blocktree
  (* type of blocks in the blockchain *)
  type block
  (* type of the transactions *)
  type transaction
  (* the type of the proof used *)
  type proof

  (* try to receive a new transaction *)
  val receive_transaction : unit -> (bool * (transaction option))
  (* try to receive a block from the network *)
  val receive_block : unit -> (bool * ((block * proof) option))
  (* write a block to the blocktree, returning the resulting blocktree *)
  val write_block : block ->blocktree ->blocktree

  (*
    (*** Five Component Framework ***)
    (* generate a block, attaching the necessary proof object *)
    val propose_block : (transaction list) ->int ->(bool * ((block * proof) option))
    (* disseminate a block (and proof) accross the network *)
    val propagate_block : (block * proof) ->unit
    (* checks if a block, proof, and contained transactions are valid *)
    val validate_block : (block * proof) ->blocktree ->bool
    (* agree on the acceptance of a block in a list of temporary validated blocks *)
    val finalize_block : block ->(block list) ->blocktree ->unit
    (* promote honest participation (to-do) *)
    val promote_honesty : unit ->unit
  *)

end
