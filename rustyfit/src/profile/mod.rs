pub use profile_type::{PROFILE_VERSION, ProfileType};

/// The `lookup` module provide a lookup function to find message representation
/// defined in the Profile.xslx.
pub mod lookup;

/// The `mesgdef` module contains all message structures that can be used as
/// intermediate representation to work with FIT message for convenience.
pub mod mesgdef;

/// The `typedef` module contains all types to work with FIT protocol representation
/// as well as to work with `mesgdef` module.
pub mod typedef;

mod profile_type;
