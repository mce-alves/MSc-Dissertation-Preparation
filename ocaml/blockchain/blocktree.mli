module type Blocktree = sig
  (* the type of the blocks in the block tree *)
  type block
  (* the type of the hash of a block *)
  type hash
  (* the type of the block tree's implementation *)
  type t

  (* the genesis block *)
  val genesis : block
  (* initial state of the block tree *)
  val init : t
  (* write a block to the tree *)
  val write_block : block ->t ->t
  (* get the head of the best chain *)
  val get_best_chain_head : t ->block
  (* check if adding <block> to the tree originates a valid chain *)
  val is_valid_chain : block ->t ->bool
  (* returns the current best chain *)
  val current_best_chain : t ->(block list)
  (* given a blocktree and two blocks (heads of different chains) *)
  (* returns which should be considered the heaviest chain *)
  val fork_choice_rule : t ->block ->block ->block
  (* ensures that adding the block to the tree won't cause cycles *)
  (* true==no cycles *)
  val no_cycles : t ->block ->bool
  (* checks if the blocktree already contains a block with the same hash *)
  val contains_hash : t ->hash ->bool

  (* operations specific to checkpoint blocktree's *)

  (* given a tree and the hash of two blocks in the tree, checks if there is a conflict *)
  (* there is a conflict when the blocks are in distinct forks (neither if descendant of the other) *)
  val check_conflict : t ->hash ->hash ->bool
  (* attempt to mark a checkpoint block (identified its the hash) as justified *)
  val mark_justified : t ->hash ->(bool * t)
  (* attempt to mark a checkpoint block (identified its the hash) as finalized *)
  val mark_finalized : t ->hash ->(bool * t)
  (* check if a checkpoint block (identified its the hash) is justified *)
  (* a checkpoint is justified if there is a chain of majority votes from the root to the checkpoint *)
  val is_justified : t ->hash ->bool
  (* check if a checkpoint block (identified its the hash) is finalized *)
  (* a checkpoint is finalized if it is justified and it has a direct child that also has a majority of votes *)
  val is_finalized : t ->hash ->bool
end
