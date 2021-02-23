open Network;;

module type Proposal = sig
  type transaction
  type block
  type proof
  (* generate a block, attaching the necessary proof object *)
  val propose_block : (transaction list) ->((block * proof) option)
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
  type communication_channels
  (* disseminate a block (and proof) accross the network *)
  val propagate_block : (block * proof) ->(communication_channels list) ->unit
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
  (NetworkOperations : Network with type block = BlockProposal.block and type transaction = BlockProposal.transaction and type proof = BlockProposal.proof)
  (CheckpointFinalization : Checkpoint with type blocktree = BlockValidation.blocktree) = struct

  let run bc txs checkpointTree receiveChannel networkChannels = begin
    let tmpBlockSet : (BlockProposal.block list ref) = ref [] in
    let rec main () =
      match (BlockProposal.propose_block !txs) with
        | Some (blk, proof) ->
          tmpBlockSet := blk::!tmpBlockSet;
          BlockPropagation.propagate_block (blk, proof) networkChannels
        | None -> ();
      match (NetworkOperations.receive_transaction receiveChannel) with
        | Some tx ->
          txs := tx::!txs
        | None -> ();
      match (NetworkOperations.receive_block receiveChannel) with
        | Some (blk, proof) ->
          begin
            match (BlockValidation.validate_block (blk, proof) !bc) with
            | true ->
              tmpBlockSet := blk::!tmpBlockSet;
              BlockPropagation.propagate_block (blk, proof) networkChannels
            | false -> ()
          end
        | None -> ();
      bc := BlockFinalization.finalize_block !tmpBlockSet !bc;
      checkpointTree := CheckpointFinalization.finalize_checkpoint !bc !checkpointTree;
      main () in
    main ()
  end

end
