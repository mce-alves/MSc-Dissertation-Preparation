module type Blocktree = sig
  (* the type of the blocks in the block tree *)
  type block
  (* the type of the hash of a block *)
  type hash
  (* the type of the block tree's implementation *)
  type blocktree

  (* the genesis block *)
  val genesis : block
  (* initial state of the block tree *)
  val init : blocktree
  (* write a block to the tree *)
  val write_block : block ->blocktree ->blocktree
  (* get the head of the best chain *)
  val get_best_chain_head : blocktree ->block
  (* check if adding <block> to the tree originates a valid chain *)
  val is_valid_chain : block ->blocktree ->bool
  (* returns the current best chain *)
  val current_best_chain : blocktree ->(block list)
  (* given a blocktree and two blocks (heads of different chains) *)
  (* returns which should be considered the heaviest chain *)
  val fork_choice_rule : blocktree ->block ->block ->block
  (* ensures that adding the block to the tree won't cause cycles *)
  (* true==no cycles *)
  val no_cycles : blocktree ->block ->bool
  (* checks if the blocktree already contains a block with the same hash *)
  val contains_hash : blocktree ->hash ->bool

  (* operations specific to checkpoint blocktree's *)

  (* given a tree and the hash of two blocks in the tree, checks if there is a conflict *)
  (* there is a conflict when the blocks are in distinct forks (neither if descendant of the other) *)
  val check_conflict : blocktree ->hash ->hash ->bool
  (* attempt to mark a checkpoint block (identified its the hash) as justified, updating the tree *)
  val mark_justified : blocktree ->hash ->(bool * blocktree)
  (* attempt to mark a checkpoint block (identified its the hash) as finalized, updating the tree *)
  val mark_finalized : blocktree ->hash ->(bool * blocktree)
  (* check if a checkpoint block (identified its the hash) is justified *)
  (* a checkpoint is justified if there is a chain of majority votes from the root to the checkpoint *)
  val is_justified : blocktree ->hash ->bool
  (* check if a checkpoint block (identified its the hash) is finalized *)
  (* a checkpoint is finalized if it is justified and it has a direct child that also has a majority of votes *)
  val is_finalized : blocktree ->hash ->bool
end
