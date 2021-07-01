
pub enum MessageError {
    InvalidHeaders,
    InvalidInstruction,
    InvalidSender,
    InvalidReceiver,
    InsufficientFunds,
    InvalidSignature,
    AccountExists,
    MissingAccount,
    InvalidSeed,
}

pub enum MessageType {
    Txn,
    ClaimHomesteaded,
    ClaimAcquired,
    Validator,
    Block,
}
