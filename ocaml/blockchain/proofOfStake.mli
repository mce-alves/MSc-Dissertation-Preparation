module type ProofOfStake = sig
  (* the type of the blocktree/blockchain *)
  type blocktree
  (* type of an asynchronous round (slot for which the block will be proposed) *)
  type round
  (* the type of the private and public seeds/keys *)
  type key
  (* the type of the hashes *)
  type hash
  (* type of the structure storing the system's users and their stake *)
  type stakes
  (* the type representing a step in the agreement protocol *)
  type step
  (* the type of the proof produced by the algorithm *)
  type proof
  (* the type that identifies a user *)
  type peer
  (* the type of a peer's vote *)
  type vote

  (* returns the users and their corresponding initial stakes (weights) *)
  val weights : stakes
  (* given a peer's private key, a round and step in the protocol, generates a hash and proof if the peer belongs to the committee for that round and step *)
  val committee_sortition : key ->round ->step ->((hash * proof) option)
  (* given a peer's private key and a round, generates a hash and proof if the peer was selected to proposer a block in that round *)
  val proposer_sortition : key ->round ->((hash * proof) option)
  (* given a peer's private key, a round and step in the protocol, and a hash*proof credential, returns whether or not the credentials are valid and belong to the public key's owner *)
  val committee_validation: key ->round ->step ->(hash * proof) ->bool
  (* given a peer's private key, a round in the protocol and a hash*proof credential, returns whether or not the credentials are valid and belong to the public key's owner *)
  val proposer_validation: key ->round ->(hash * proof) ->bool


  (** PoS Checkpoint Operations (main source for these signatures is Casper FFG) **)

  (* given the current blocktree, checkpointtree, user and round, creates a vote *)
  (* representing which block the user believes should be the next checkpoint *)
  val create_checkpoint_vote : blocktree ->blocktree ->peer ->round ->vote
  (* cheks if a vote is valid -> compare against slashing rules *)
  val validate_vote : blocktree ->vote ->bool
  (* process a received vote, and store it if it is valid (also store the block in the tree, if it isn't there yet) *)
  val process_checkpoint_vote : blocktree ->vote ->(vote list) ->((vote list) * blocktree)
  (* given the list of received votes, produces the pair (source block * target block) *)
  (* representing the achieved majority link, if it exists *)
  (* upon success, the source block can be marked as finalized, and the target as justified, updating the checkpoint tree *)
  val process_votes : blocktree ->(vote list) -> (((hash * hash) option) * blocktree)
end
