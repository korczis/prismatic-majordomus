//! Majordomus Fabric's local contract: what a client and the opt-in host agent say to each
//! other over the user's socket (ADR 0199, amendment of 2026-11-24; phase P02).
//!
//! This module is the contract and nothing that runs it. It defines the frame
//! ([`frame`]), the envelope and the messages ([`message`]), the identifiers
//! ([`ids`]), the protocol versions ([`version`]), the refusals with their stable codes
//! ([`refusal`]) and the local transport's bounds ([`transport`]). No listener, service,
//! socket or process is started by anything here: those are the phases after this one, and
//! they drive [`Conversation`] rather than restating its rules.
//!
//! Three decisions shape it, each the amendment's:
//!
//! - **Being on the socket is not authority.** A session is bound to one context and one
//!   connection; every call names both its session and its context, and runs only when the
//!   two agree with what the host holds ([`Call::admit`]). The checkout root a client sends
//!   is where the host starts its own resolution, never proof of anything.
//! - **No second registry and no second identity.** Capabilities are the registry's, by
//!   [`crate::capability::CapabilityId`], and a call is admitted against the registry's
//!   descriptor. A context's id is the checkout's ([`crate::session::CheckoutId`]), its
//!   repository git's common directory ([`crate::session::RepositoryId`]), the host the
//!   mesh's node and instance. The handler's own checks still run after admission.
//! - **What ran is always said.** Every refusal carries a [`Delivery`]: not run, ran, or
//!   unknown. Only a capability whose declared policy is idempotent is sent again after an
//!   unknown outcome, the rule [`crate::mcp::bridge`] already keeps.
//!
//! `docs/FABRIC.md` is the reader's account of the contract.
//!
//! ```
//! use majordomus_cli::fabric::{ClientFrame, Conversation, HostIdentity, HostOffer};
//! use majordomus_cli::mesh::identity::NodeIdentity;
//!
//! let node = NodeIdentity::ephemeral().unwrap();
//! let mut conversation = Conversation::new(HostOffer::this_build(HostIdentity::of(&node.public)));
//! let hello = ClientFrame::decode(
//!     br#"{"request":1,"message":{"type":"hello","protocol":{"min":1,"max":1}}}"#,
//! ).unwrap();
//! let welcome = conversation.receive(&hello).unwrap().expect("a hello is welcomed");
//! assert_eq!(welcome.protocol.0, 1);
//! ```

pub mod frame;
pub mod ids;
pub mod message;
pub mod refusal;
pub mod transport;
pub mod version;

pub use ids::{
    CheckoutRoot, ClientLabel, ContextDescriptor, ContextId, FeatureName, FeatureSet, RequestId,
    SessionId,
};
pub use message::{
    admit_session, negotiate, Ack, Attach, Attached, Call, Cancel, ClientFrame, ClientMessage,
    Conversation, Deadline, Detach, HealthQuery, HealthReport, Hello, HostFrame, HostIdentity,
    HostMessage, HostOffer, Limits, Negotiated, Output, Rejected, SessionBinding, Welcome,
};
pub use refusal::{Delivery, Refusal, RefusalCode};
pub use version::{ProtocolRange, ProtocolVersion, PROTOCOL};
