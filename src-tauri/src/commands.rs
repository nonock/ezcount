//! The app's commands, as the frontend calls them (`src/bindings.ts` is generated from them).
//! Most are a line around the function of the same name in `ezcount_core::api`.

pub(crate) mod account;
pub(crate) mod expenses;
pub(crate) mod groups;
pub(crate) mod native;
