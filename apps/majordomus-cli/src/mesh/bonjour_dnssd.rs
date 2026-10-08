//! The mDNSResponder backend of the Bonjour provider: the system's DNS-SD service on
//! macOS, reached through the C API of `<dns_sd.h>`, which lives in libSystem and needs
//! no linking of its own.
//!
//! One registration (`DNSServiceRegister`), whose TXT record is replaced in place for every
//! new envelope (`DNSServiceUpdateRecord`: one announcement, no withdrawal); one browse
//! (`DNSServiceBrowse`); and, for every instance the browse reports, one standing query for
//! its TXT record (`DNSServiceQueryRecord`), which answers at once and again whenever the
//! record changes. The instance's address and port are never resolved: the envelope
//! carries the endpoints, signed, and an unsigned SRV record would only be a second
//! account of them.
//!
//! Everything the daemon says arrives through callbacks that run inside
//! `DNSServiceProcessResult`, which this backend calls only from [`DnsSd::poll`], on the
//! one thread that owns it, and only for a reference `poll(2)` reported readable — so no
//! call blocks beyond the wait it was given. A callback does nothing but copy what it was
//! handed into an inbox; the references are opened and closed outside the callbacks.
//!
//! Every reference is deallocated when the value is dropped, which is what withdraws the
//! registration: the daemon sends the goodbye.

use std::collections::BTreeMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::time::Duration;

use super::bonjour::{txt_rdata, txt_strings, Advertised, DnsSd, Found, DOMAIN};

/// `DNSServiceRef`: an opaque connection to the daemon.
type ServiceRef = *mut c_void;

/// `kDNSServiceFlagsAdd`: the callback reports something that appeared, not one that left.
const FLAG_ADD: u32 = 0x2;
/// `kDNSServiceType_TXT`.
const TYPE_TXT: u16 = 16;
/// `kDNSServiceClass_IN`.
const CLASS_IN: u16 = 1;
/// `kDNSServiceMaxDomainName`: the buffer `DNSServiceConstructFullName` writes into.
const MAX_DOMAIN_NAME: usize = 1009;

/// The most instances whose TXT record is watched at once. A segment is hostile input:
/// whoever registers a thousand instances gets sixty-four queries and no more.
const MAX_INSTANCES: usize = 64;

type BrowseReply = unsafe extern "C" fn(
    ServiceRef,
    u32,
    u32,
    i32,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut c_void,
);

type QueryReply = unsafe extern "C" fn(
    ServiceRef,
    u32,
    u32,
    i32,
    *const c_char,
    u16,
    u16,
    u16,
    *const c_void,
    u32,
    *mut c_void,
);

extern "C" {
    fn DNSServiceRegister(
        sd_ref: *mut ServiceRef,
        flags: u32,
        interface_index: u32,
        name: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
        host: *const c_char,
        port: u16,
        txt_len: u16,
        txt_record: *const c_void,
        callback: *const c_void,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceUpdateRecord(
        sd_ref: ServiceRef,
        record_ref: *mut c_void,
        flags: u32,
        rdlen: u16,
        rdata: *const c_void,
        ttl: u32,
    ) -> i32;
    fn DNSServiceBrowse(
        sd_ref: *mut ServiceRef,
        flags: u32,
        interface_index: u32,
        regtype: *const c_char,
        domain: *const c_char,
        callback: BrowseReply,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceQueryRecord(
        sd_ref: *mut ServiceRef,
        flags: u32,
        interface_index: u32,
        fullname: *const c_char,
        rrtype: u16,
        rrclass: u16,
        callback: QueryReply,
        context: *mut c_void,
    ) -> i32;
    fn DNSServiceConstructFullName(
        full_name: *mut c_char,
        service: *const c_char,
        regtype: *const c_char,
        domain: *const c_char,
    ) -> i32;
    fn DNSServiceRefSockFD(sd_ref: ServiceRef) -> c_int;
    fn DNSServiceProcessResult(sd_ref: ServiceRef) -> i32;
    fn DNSServiceRefDeallocate(sd_ref: ServiceRef);
}

/// What the callbacks copied out of the daemon's answers, in arrival order.
enum Event {
    /// The browse reported an instance (its full name), or — `false` — its departure.
    Instance(CString, bool),
    /// A TXT record of an instance, as its wire bytes.
    Txt(CString, Vec<u8>),
    /// The browse reported an error.
    Failed(i32),
}

/// The mDNSResponder backend: the references this runtime holds.
pub(crate) struct Responder {
    /// The registration, with the name, type and port it was made for.
    registered: Option<(ServiceRef, String, String, u16)>,
    browsing: Option<ServiceRef>,
    /// One standing TXT query per instance, with how many interfaces report the instance.
    queries: BTreeMap<CString, (ServiceRef, usize)>,
    /// Where the callbacks write. Owned through this pointer and freed in `drop`.
    inbox: *mut Vec<Event>,
}

// SAFETY: a DNSServiceRef has no thread affinity; it must only not be used from two
// threads at once. `Responder` is not `Sync`, every method takes `&mut self`, and the
// provider moves the value into the one thread that then owns it for its whole life.
unsafe impl Send for Responder {}

impl Responder {
    /// A backend that has asked the daemon for nothing yet.
    pub(crate) fn new() -> Self {
        Responder {
            registered: None,
            browsing: None,
            queries: BTreeMap::new(),
            inbox: Box::into_raw(Box::new(Vec::new())),
        }
    }

    fn withdraw(&mut self) {
        if let Some((reference, ..)) = self.registered.take() {
            // SAFETY: the reference came from DNSServiceRegister and is deallocated once.
            unsafe { DNSServiceRefDeallocate(reference) };
        }
    }

    /// Open or close TXT queries for what the browse reported, and turn the TXT records
    /// that arrived into instances.
    fn settle(&mut self, events: Vec<Event>) -> Result<Vec<Found>, String> {
        let mut found = Vec::new();
        for event in events {
            match event {
                Event::Failed(code) => return Err(format!("DNSServiceBrowse reported {code}")),
                Event::Instance(name, true) => {
                    if let Some((_, interfaces)) = self.queries.get_mut(&name) {
                        *interfaces += 1;
                    } else if self.queries.len() < MAX_INSTANCES {
                        let mut reference: ServiceRef = std::ptr::null_mut();
                        // SAFETY: `name` is a NUL-terminated full name that outlives the
                        // call; the callback and its context (the inbox) outlive the
                        // reference, which `drop` deallocates before it frees the inbox.
                        let code = unsafe {
                            DNSServiceQueryRecord(
                                &mut reference,
                                0,
                                0,
                                name.as_ptr(),
                                TYPE_TXT,
                                CLASS_IN,
                                on_txt,
                                self.inbox.cast(),
                            )
                        };
                        if code == 0 {
                            self.queries.insert(name, (reference, 1));
                        }
                    }
                }
                Event::Instance(name, false) => {
                    let gone = match self.queries.get_mut(&name) {
                        Some((_, interfaces)) => {
                            *interfaces -= 1;
                            *interfaces == 0
                        }
                        None => false,
                    };
                    if let Some((reference, _)) = gone.then(|| self.queries.remove(&name)).flatten()
                    {
                        // SAFETY: the reference came from DNSServiceQueryRecord, was just
                        // taken out of the map, and is deallocated once.
                        unsafe { DNSServiceRefDeallocate(reference) };
                    }
                }
                Event::Txt(name, rdata) => {
                    if let Some(txt) = txt_strings(&rdata) {
                        found.push(Found {
                            name: name.to_string_lossy().into_owned(),
                            txt,
                        });
                    }
                }
            }
        }
        Ok(found)
    }
}

impl DnsSd for Responder {
    fn mechanism(&self) -> &'static str {
        "mDNSResponder"
    }

    fn publish(&mut self, advertised: &Advertised) -> Result<(), String> {
        let rdata = txt_rdata(&advertised.txt).ok_or("a TXT string exceeds 255 bytes")?;
        let length = u16::try_from(rdata.len()).map_err(|_| "the TXT record is too large")?;
        if let Some((reference, name, service, port)) = &self.registered {
            if (name, service, port) == (&advertised.name, &advertised.service, &advertised.port) {
                // SAFETY: the reference is the live registration; a null record names its
                // primary TXT record; `rdata` outlives the call, which copies it.
                let code = unsafe {
                    DNSServiceUpdateRecord(
                        *reference,
                        std::ptr::null_mut(),
                        0,
                        length,
                        rdata.as_ptr().cast(),
                        0,
                    )
                };
                return match code {
                    0 => Ok(()),
                    code => Err(format!("DNSServiceUpdateRecord returned {code}")),
                };
            }
        }
        self.withdraw();
        let text =
            |s: &str| CString::new(s).map_err(|_| "a NUL byte in the registration".to_string());
        let (name, service, domain) = (
            text(&advertised.name)?,
            text(&advertised.service)?,
            text(DOMAIN)?,
        );
        let mut reference: ServiceRef = std::ptr::null_mut();
        // SAFETY: every pointer is a NUL-terminated string or a buffer of the stated
        // length that outlives the call; no callback is given, so no context is kept.
        let code = unsafe {
            DNSServiceRegister(
                &mut reference,
                0,
                0,
                name.as_ptr(),
                service.as_ptr(),
                domain.as_ptr(),
                std::ptr::null(),
                advertised.port.to_be(),
                length,
                rdata.as_ptr().cast(),
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if code != 0 {
            return Err(format!("DNSServiceRegister returned {code}"));
        }
        self.registered = Some((
            reference,
            advertised.name.clone(),
            advertised.service.clone(),
            advertised.port,
        ));
        Ok(())
    }

    fn browse(&mut self, service: &str) -> Result<(), String> {
        let text =
            |s: &str| CString::new(s).map_err(|_| "a NUL byte in the service type".to_string());
        let (service, domain) = (text(service)?, text(DOMAIN)?);
        let mut reference: ServiceRef = std::ptr::null_mut();
        // SAFETY: the strings outlive the call; the callback and its context (the inbox)
        // outlive the reference, which `drop` deallocates before it frees the inbox.
        let code = unsafe {
            DNSServiceBrowse(
                &mut reference,
                0,
                0,
                service.as_ptr(),
                domain.as_ptr(),
                on_instance,
                self.inbox.cast(),
            )
        };
        if code != 0 {
            return Err(format!("DNSServiceBrowse returned {code}"));
        }
        self.browsing = Some(reference);
        Ok(())
    }

    fn poll(&mut self, wait: Duration) -> Result<Vec<Found>, String> {
        let Some(browsing) = self.browsing else {
            return Err("not browsing".into());
        };
        let references: Vec<ServiceRef> = std::iter::once(browsing)
            .chain(self.queries.values().map(|(reference, _)| *reference))
            .collect();
        let mut waits: Vec<libc::pollfd> = references
            .iter()
            .map(|reference| libc::pollfd {
                // SAFETY: every reference in the list is live.
                fd: unsafe { DNSServiceRefSockFD(*reference) },
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        // SAFETY: `waits` is a valid array of its stated length for the whole call.
        let ready = unsafe {
            libc::poll(
                waits.as_mut_ptr(),
                waits.len() as libc::nfds_t,
                wait.as_millis() as c_int,
            )
        };
        let mut ended: Vec<ServiceRef> = Vec::new();
        if ready > 0 {
            for (reference, waited) in references.iter().zip(&waits) {
                if waited.revents == 0 {
                    continue;
                }
                // SAFETY: the reference is live and its socket is readable (or closed by
                // the daemon), so this returns without waiting; the callbacks it runs
                // only push to the inbox, which nothing else borrows meanwhile.
                let code = unsafe { DNSServiceProcessResult(*reference) };
                if code != 0 && *reference == browsing {
                    return Err(format!("the connection to mDNSResponder ended ({code})"));
                }
                if code != 0 {
                    ended.push(*reference);
                }
            }
        }
        // A query the daemon ended stays readable for ever: left in the set it would turn
        // every later wait into a spin. It is closed here, and the browse reports the
        // instance again if it is still there.
        self.queries.retain(|_, (reference, _)| {
            let gone = ended.contains(reference);
            if gone {
                // SAFETY: the reference came from DNSServiceQueryRecord, leaves the map
                // with this call, and is deallocated once.
                unsafe { DNSServiceRefDeallocate(*reference) };
            }
            !gone
        });
        // SAFETY: the inbox is a live allocation only the callbacks write to, and none
        // runs outside DNSServiceProcessResult, which has returned.
        let events = unsafe { std::mem::take(&mut *self.inbox) };
        self.settle(events)
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.withdraw();
        let queries = std::mem::take(&mut self.queries);
        for reference in queries
            .into_values()
            .map(|(r, _)| r)
            .chain(self.browsing.take())
        {
            // SAFETY: each reference is live and deallocated exactly once, here.
            unsafe { DNSServiceRefDeallocate(reference) };
        }
        // SAFETY: the pointer came from Box::into_raw in `new`; every reference whose
        // callback could write to it has been deallocated above.
        drop(unsafe { Box::from_raw(self.inbox) });
    }
}

/// The browse callback: an instance appeared or left. Copies its full name into the inbox.
///
/// SAFETY (of the callers' contract): the daemon library calls this from within
/// `DNSServiceProcessResult` with NUL-terminated strings valid for the call, and with the
/// context given to `DNSServiceBrowse`, which is the inbox.
unsafe extern "C" fn on_instance(
    _reference: ServiceRef,
    flags: u32,
    _interface: u32,
    error: i32,
    name: *const c_char,
    regtype: *const c_char,
    domain: *const c_char,
    context: *mut c_void,
) {
    // SAFETY: see the function's contract above.
    unsafe {
        let inbox = &mut *context.cast::<Vec<Event>>();
        if error != 0 {
            inbox.push(Event::Failed(error));
            return;
        }
        let mut full = [0 as c_char; MAX_DOMAIN_NAME];
        if DNSServiceConstructFullName(full.as_mut_ptr(), name, regtype, domain) == 0 {
            let full = CStr::from_ptr(full.as_ptr()).to_owned();
            inbox.push(Event::Instance(full, flags & FLAG_ADD != 0));
        }
    }
}

/// The query callback: a TXT record of an instance was read, or replaced. Copies the
/// record's bytes into the inbox; a record that went away is not news.
///
/// SAFETY (of the callers' contract): as for [`on_instance`]; `rdata` points at `rdlen`
/// bytes valid for the call.
unsafe extern "C" fn on_txt(
    _reference: ServiceRef,
    flags: u32,
    _interface: u32,
    error: i32,
    fullname: *const c_char,
    rrtype: u16,
    _rrclass: u16,
    rdlen: u16,
    rdata: *const c_void,
    _ttl: u32,
    context: *mut c_void,
) {
    if error != 0
        || flags & FLAG_ADD == 0
        || rrtype != TYPE_TXT
        || rdata.is_null()
        || fullname.is_null()
    {
        return;
    }
    // SAFETY: see the function's contract above.
    unsafe {
        let inbox = &mut *context.cast::<Vec<Event>>();
        let bytes = std::slice::from_raw_parts(rdata.cast::<u8>(), usize::from(rdlen)).to_vec();
        inbox.push(Event::Txt(CStr::from_ptr(fullname).to_owned(), bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // What can be held without the daemon: everything up to the first request, and what
    // happens to answers once they are in the inbox. The requests themselves are the
    // ignored test `the_real_service_registers_this_runtime_and_browses_its_envelope_back`.

    #[test]
    fn a_responder_that_asked_for_nothing_holds_nothing_and_is_not_browsing() {
        let mut responder = Responder::new();
        assert_eq!(responder.mechanism(), "mDNSResponder");
        assert!(responder.registered.is_none() && responder.browsing.is_none());
        assert_eq!(
            responder.poll(Duration::from_millis(1)).unwrap_err(),
            "not browsing"
        );
        // refused before the daemon is asked: a string no length byte can state, and NULs
        let advertised = |name: &str, txt: Vec<Vec<u8>>| Advertised {
            name: name.into(),
            service: "_mjt._tcp".into(),
            port: 1,
            txt,
        };
        assert_eq!(
            responder
                .publish(&advertised("n", vec![vec![b'x'; 256]]))
                .unwrap_err(),
            "a TXT string exceeds 255 bytes"
        );
        assert_eq!(
            responder.publish(&advertised("n\0", vec![])).unwrap_err(),
            "a NUL byte in the registration"
        );
        assert_eq!(
            responder.browse("_mjt\0._tcp").unwrap_err(),
            "a NUL byte in the service type"
        );
        assert!(responder.registered.is_none() && responder.browsing.is_none());
    }

    #[test]
    fn what_arrived_is_settled_into_instances_and_a_failed_browse_ends_it() {
        let name = || CString::new("majordomus-641bdb94-01234567._mjt._tcp.local.").unwrap();
        let mut responder = Responder::new();
        let found = responder
            .settle(vec![
                // a record that is no TXT record at all is dropped, not guessed at
                Event::Txt(name(), b"\x09short".to_vec()),
                Event::Txt(name(), b"\x09txtvers=1\x03n=0".to_vec()),
                // an instance nobody was watching leaves, and nothing is deallocated
                Event::Instance(name(), false),
            ])
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].name,
            "majordomus-641bdb94-01234567._mjt._tcp.local."
        );
        assert_eq!(found[0].txt, [b"txtvers=1".to_vec(), b"n=0".to_vec()]);
        assert!(responder.queries.is_empty());
        assert_eq!(
            responder.settle(vec![Event::Failed(-65563)]).unwrap_err(),
            "DNSServiceBrowse reported -65563"
        );
    }
}
