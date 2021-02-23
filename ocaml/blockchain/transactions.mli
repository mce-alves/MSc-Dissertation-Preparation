module type Transactions = sig
  (* type of the hash of a transaction *)
  type hash
  (* type representation of a transaction *)
  type transaction

  (* initialize empty transaction list *)
  val init : (transaction list)
  (* validate a transaction *)
  val validate : transaction ->bool
  (* adds a transaction to a list of transactions, returning the resulting list *)
  val add_transaction : transaction ->(transaction list) ->(transaction list)
  (* computes the hash of a transaction *)
  val compute_hash : transaction ->hash
end
