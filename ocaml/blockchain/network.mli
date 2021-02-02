module type Network = sig
  (* the type of blocks included in the messages *)
  type block
  (* the type of the messages sent in the network *)
  type message
  (* the type of the nodes in the network *)
  type node
  (* the type of the networks implementation *)
  type t

  (* minimum delay when sending a message *)
  val min_delay : float
  (* maximum delay when sending a message *)
  val max_delay : float
  (* chance to fail when sending a message *)
  val fail_chance : float
  (* sends a message to all known nodes in the network t *)
  val broadcast_block : block ->t ->unit
  (* sends a message to some select nodes in the network t *)
  val gossip_block : block ->t ->unit
  (* relay a message to all known nodes except the sender *)
  val relay_block : block ->t ->unit
  (* receives the upper,lower delay bounds, and returns a delay in that interval *)
  val compute_delay : float ->float
  (* wraps a block in a message (sender, block, receiver) *)
  val create_msg_wrapper : node ->block ->node ->message
  (* create a wrapper for a block request message, given a block hash *)
  val create_req_wrapper : node ->int ->node ->message
  (* extract block wrapped in a message *)
  val extract_block_from_msg : message ->block
end
