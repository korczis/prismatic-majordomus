//! Which declared host a derive goes to: of the hosts that answer and have at least their
//! free-space minimum, the one with the fewest derives running or waiting; a tie goes to the
//! host declared first.

use serde::Serialize;

use super::declaration::{OffloadDeclaration, OffloadHost};
use super::queue::{Observation, Queue};
use super::remote::Remote;

/// Where a host stands for the next derive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// It answers and has the space: it may be chosen.
    Eligible,
    /// It did not answer, or answered something the queue cannot read.
    Unreachable,
    /// It has less free space than its minimum.
    UnderMinimum,
    /// It answered without a free-space figure, and unknown is never enough.
    UnknownSpace,
}

/// One host's measurement and verdict.
#[derive(Debug, Clone, Serialize)]
pub struct HostReport {
    /// The declared id.
    pub id: String,
    /// `user@host`.
    pub address: String,
    /// The minimum that applies to it, GiB.
    pub min_free_gib: u64,
    /// GiB free, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub free_gib: Option<u64>,
    /// Derives running or waiting, when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<usize>,
    /// The verdict.
    pub standing: Standing,
    /// Why, in words, when it is not eligible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether this is the host a derive would take.
    pub chosen: bool,
}

/// The remote a declared host is reached by.
pub fn remote_of(host: &OffloadHost) -> Remote {
    Remote::ssh(&host.address, host.identity_file.as_deref(), &host.id)
}

/// Judge every host from its observation (in declaration order) and mark the one chosen.
pub fn choose(
    d: &OffloadDeclaration,
    observations: Vec<Result<Observation, String>>,
) -> Vec<HostReport> {
    let mut reports: Vec<HostReport> = d
        .hosts
        .iter()
        .zip(observations)
        .map(|(h, o)| {
            let min = h.min_free_gib(d);
            let mut r = HostReport {
                id: h.id.clone(),
                address: h.address.clone(),
                min_free_gib: min,
                free_gib: None,
                load: None,
                standing: Standing::Eligible,
                reason: None,
                chosen: false,
            };
            match o {
                Err(e) => {
                    r.standing = Standing::Unreachable;
                    r.reason = Some(e);
                }
                Ok(o) => {
                    r.free_gib = o.free_gib();
                    r.load = Some(o.load());
                    match r.free_gib {
                        None => {
                            r.standing = Standing::UnknownSpace;
                            r.reason = Some("df reported no free space figure".into());
                        }
                        Some(f) if f < min => {
                            r.standing = Standing::UnderMinimum;
                            r.reason = Some(format!("{f} GiB free, under {min}"));
                        }
                        Some(_) => {}
                    }
                }
            }
            r
        })
        .collect();
    let best = reports
        .iter()
        .enumerate()
        .filter(|(_, r)| r.standing == Standing::Eligible)
        .min_by_key(|(i, r)| (r.load.unwrap_or(usize::MAX), *i))
        .map(|(i, _)| i);
    if let Some(i) = best {
        reports[i].chosen = true;
    }
    reports
}

/// Observe every declared host, one after another, and judge them.
pub fn probe(d: &OffloadDeclaration) -> Vec<HostReport> {
    let observations = d
        .hosts
        .iter()
        .map(|h| {
            let remote = remote_of(h);
            Queue {
                remote: &remote,
                directory: &d.queue.directory,
                lock: &d.queue.lock,
                poll: std::time::Duration::from_secs(d.queue.poll_seconds),
                wait_max: std::time::Duration::from_secs(d.queue.wait_max_seconds),
            }
            .observe()
            .map_err(|e| e.to_string())
        })
        .collect();
    choose(d, observations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaration(n: usize) -> OffloadDeclaration {
        let hosts: Vec<_> = (0..n)
            .map(|i| serde_json::json!({ "id": format!("h{i}"), "address": format!("u@h{i}") }))
            .collect();
        serde_json::from_value(serde_json::json!({
            "schema": "derive-offload/v1", "kind": "derive-offload", "id": "d", "hosts": hosts
        }))
        .unwrap()
    }

    fn o(text: &str) -> Result<Observation, String> {
        Ok(Observation::parse(text).unwrap())
    }

    const ROOMY: &str = "free 104857600\n";

    #[test]
    fn the_least_loaded_eligible_host_is_chosen() {
        let reports = choose(
            &declaration(3),
            vec![
                o(&format!("{ROOMY}ticket 1-1-a alive\nticket 2-2-b alive\n")),
                o(&format!("{ROOMY}lock\nholder 3 alive\n")),
                o(&format!("{ROOMY}ticket 1-1-a alive\nlock\n")),
            ],
        );
        let chosen: Vec<_> = reports
            .iter()
            .filter(|r| r.chosen)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(chosen, ["h1"]);
        assert_eq!(reports[0].load, Some(2));
        assert_eq!(reports[2].load, Some(2));
    }

    #[test]
    fn a_tie_goes_to_the_host_declared_first() {
        let reports = choose(&declaration(2), vec![o(ROOMY), o(ROOMY)]);
        assert!(reports[0].chosen && !reports[1].chosen);
    }

    #[test]
    fn hosts_that_cannot_take_a_derive_are_skipped_and_say_why() {
        let reports = choose(
            &declaration(4),
            vec![
                Err("ssh: connect to host timed out".into()),
                o("free 9437184\n"), // 9 GiB, under the default 10
                o("free unknown\n"),
                o(&format!(
                    "{ROOMY}ticket 1-1-a alive\nticket 2-2-b alive\nticket 3-3-c alive\n"
                )),
            ],
        );
        assert_eq!(reports[0].standing, Standing::Unreachable);
        assert_eq!(reports[1].standing, Standing::UnderMinimum);
        assert!(reports[1]
            .reason
            .as_deref()
            .unwrap()
            .contains("9 GiB free, under 10"));
        assert_eq!(reports[2].standing, Standing::UnknownSpace);
        assert!(
            reports[3].chosen,
            "the busiest host is still better than none"
        );
    }

    #[test]
    fn a_host_may_carry_its_own_minimum() {
        let mut d = declaration(2);
        d.hosts[0].own_min_free_gib = Some(200);
        let reports = choose(&d, vec![o(ROOMY), o(ROOMY)]);
        assert_eq!(reports[0].standing, Standing::UnderMinimum);
        assert!(reports[1].chosen);
    }

    #[test]
    fn no_eligible_host_chooses_nothing() {
        let reports = choose(&declaration(2), vec![Err("down".into()), o("free 0\n")]);
        assert!(reports.iter().all(|r| !r.chosen));
    }
}
