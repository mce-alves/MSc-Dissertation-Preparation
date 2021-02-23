module type ProofOfStake = sig
  (* the type of the blocktree/blockchain *)
  type blocktree
  (* type of an asynchronous round (slot for which the block will be proposed) *)
  type round
  (* the type representing a role in the protocol (committee member, etc.) *)
  type role
  (* the type of the private and public seeds *)
  type seed
  (* the type of the hashes *)
  type hash
  (* type of the structure storing the system's users and their stake *)
  type stakes
  (* the type of the threshold used for sortition *)
  type threshold
  (* the type of the proof produced by the algorithm *)
  type proof
  (* the type that identifies a user *)
  type user
  (* the type of a peer's vote *)
  type vote

  (* returns the users and their corresponding stakes (weights) *)
  val weights : stakes
  (* cryptographic sorition implementation *)
  (* selects a random set of users (committee) according to their weight (stake) *)
  val sortition : stakes ->round ->threshold ->(user list)
  (* verifiable random function *)
  (* receives a user's public seed, role and round, and returns its hash and proof *)
  (* used to verify if a hash and proof of capabilities belong to a given user *)
  (* can also be used to verify if the user belongs to this round's committee *)
  val vrf : seed ->role ->round ->(hash * proof)
  (* checks if a user is the winner (can propose in the current round) *)
  val check_winner : user ->round ->threshold ->bool


  (** PoS Checkpoint Operations (main source for these signatures is Casper FFG) **)

  (* given the current blocktree, checkpointtree, user and round, creates a vote *)
  (* representing which block the user believes should be the next checkpoint *)
  val create_checkpoint_vote : blocktree ->blocktree ->user ->round ->vote
  (* cheks if a vote is valid -> compare against slashing rules *)
  val validate_vote : blocktree ->vote ->bool
  (* process a received vote, and store it if it is valid (also store the block in the tree, if it isn't there yet) *)
  val process_checkpoint_vote : blocktree ->vote ->(vote list) ->((vote list) * blocktree)
  (* given the list of received votes, produces the pair (source block * target block) *)
  (* representing the achieved majority link, if it exists *)
  (* upon success, the source block can be marked as finalized, and the target as justified, updating the checkpoint tree *)
  val process_votes : blocktree ->(vote list) -> (((hash * hash) option) * blocktree)
end
