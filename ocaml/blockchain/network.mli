module type Network = sig
  (* the type of blocks included in the messages *)
  type block
  (* the type of proof that is associated with a block, and gets propagated in the network *)
  type proof
  (* the type of a transaction *)
  type transaction
  (* the type of the messages sent in the network *)
  type message
  (* the communications channels that one peer uses to receive its messages, and others use to send messages to it *)
  type communication_channels

  (* minimum delay when sending a message *)
  val min_delay : float
  (* maximum delay when sending a message *)
  val max_delay : float
  (* chance to fail when sending a message *)
  val fail_chance : float
  (* sends a message to the nodes that own the communication channels present in the list *)
  (* A message can be a block*proof, transaction, etc *)
  (* the appropriate channel for the message type will be selected via pattern matchin *)
  val send_message : message ->(communication_channels list) ->unit
  (* try to receive a new transaction *)
  val receive_transaction : communication_channels ->(transaction option)
  (* try to receive a block from the network *)
  val receive_block : communication_channels ->((block * proof) option)
  (* uses the min,max delay bounds, and returns a delay in that interval *)
  val compute_delay : float
end
