module type Transactions = sig
  (* type of the hash of a transaction *)
  type hash
  (* type representation of a transaction *)
  type t

  (* initialize empty transaction list *)
  val init : (t list)
  (* validate a transaction *)
  val validate : t ->bool
  (* adds a transaction to a list of transactions, returning the resulting list *)
  val add_transaction : t ->(t list) ->(t list)
  (* computes the hash of a transaction *)
  val compute_hash : t ->hash
end
