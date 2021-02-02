(* not generalized -> only based on Algorand Agreement in communication setting 1 *)
module type Agreement = sig
  (* the type of the value we want agreement on *)
  type v
  (* the type of a period (loop iteration) *)
  type period
  (* the type of a user's credentials *)
  type credential
  (* the current step of the period *)
  type step
  (* the different types of votes exchanged in the protocol *)
  type next_vote
  type cert_vote
  type soft_vote

  (* returns the leader for the period, given a list of credentials for that period *)
  val get_leader : period ->(credential list) ->credential

  (* given a period and the list of next-votes, produces the value to be propagated, and associated credentials *)
  val proposal_step : (next_vote list) ->period ->(v * credential)
  (* given a period and the list of next-votes, produces a soft_vote *)
  val filtering_step : (next_vote list) ->period ->soft_vote
  (* given the list of soft-votes received that match some criteria, produces a cert_vote, else None *)
  val certifying_step : (soft_vote list) ->period ->(cert_vote option)
  (* given the period and a value v, produces a next_vote for v *)
  val first_finishing_step : period ->v ->next_vote
  (* given the list of soft-votes received that match some criteria, and the period, produces a next_vote *)
  val second_finishing_step : (soft_vote list) ->period ->next_vote
  (* checks if cert-votes received match the halting criteria *)
  val halt_condition : (cert_vote list) ->bool
end
