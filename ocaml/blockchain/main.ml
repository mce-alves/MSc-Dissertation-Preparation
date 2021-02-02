open Node;;

module type Proposal = sig
  type transaction
  type block
  type proof
  (* generate a block, attaching the necessary proof object *)
  val propose_block : (transaction list) ->int ->(bool * ((block * proof) option))
end

module type Validation = sig
  type block
  type proof
  type blocktree
  (* checks if a block, proof, and contained transactions are valid *)
  val validate_block : (block * proof) ->blocktree ->bool
end

module type Propagation = sig
  type block
  type proof
  (* disseminate a block (and proof) accross the network *)
  val propagate_block : (block * proof) ->unit
end

module type Finalization = sig
  type block
  type blocktree
  (* agree on the acceptance of validated block *)
  val finalize_block : (block list) ->blocktree ->blocktree
end

module type Incentive = sig
  (* promote honest participation (to-do) *)
  val promote_honesty : unit ->unit
end

module type Checkpoint = sig
  type blocktree
  type checkpointtree
  (* executes the PoS checkpoint consensus protocol *)
  (* receives the current blocktree and checkpoint tree, and returns the updated checkpoint tree *)
  (* this module will include the blocktree and ProofOfStake modules, as they contain the operations will be used *)
  (* in this function *)
  val finalize_checkpoint : blocktree ->checkpointtree ->checkpointtree
end

module Main
  (BlockProposal : Proposal)
  (BlockValidation : Validation with type block = BlockProposal.block and type proof = BlockProposal.proof)
  (BlockPropagation : Propagation with type block = BlockProposal.block and type proof = BlockProposal.proof)
  (BlockFinalization : Finalization with type block = BlockProposal.block and type blocktree = BlockValidation.blocktree)
  (IncentiveMechanism : Incentive)
  (N : Node with type block = BlockProposal.block and type proof = BlockProposal.proof and type blocktree = BlockFinalization.blocktree)
  (CheckpointFinalization : Checkpoint with type blocktree = N.blocktree) = struct

  let run bc txs checkpointTree = begin
    let tmpBlockSet : (N.block list ref) = ref [] in
    let rec main () =
      match (BlockProposal.propose_block txs 0) with
        | (true, Some (blk, proof)) ->
          tmpBlockSet := blk::!tmpBlockSet;
          BlockPropagation.propagate_block (blk, proof)
        | (_, _) -> ();
      match (N.receive_block ()) with
        | (true, Some (blk, proof)) ->
          begin
            match (BlockValidation.validate_block (blk, proof) !bc) with
            | true ->
              tmpBlockSet := blk::!tmpBlockSet;
              BlockPropagation.propagate_block (blk, proof)
            | false -> ()
          end
        | (_, _) -> ();
      bc := BlockFinalization.finalize_block !tmpBlockSet !bc;
      checkpointTree := CheckpointFinalization.finalize_checkpoint !bc !checkpointTree;
      main () in
    main ()
  end

end
