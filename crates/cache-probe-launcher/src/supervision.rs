//! Independent, monotonic observations of a child process and its output.
//! The caller supplies elapsed time from its OWN clock and facts observed by
//! process/pipe/save watchers. This does not implement a host watchdog.
#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessObservation {
    pub exited: bool,
    pub stdout_eof: bool,
    pub received_bytes: u64,
    /// Opaque receiver-computed content fingerprint, not a signing credential.
    pub received_hash: Option<[u8; 32]>,
    pub forwarding_complete: bool,
    pub saved: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorError {
    InvalidDeadline,
    ClockRegressed,
    FactRegressed,
    InvalidEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    Pending,
    /// Only process/receiver completion; never driver STOP, capture validity,
    /// successful forwarding at the application layer, or durable persistence.
    Completed,
    DeadlineUnconfirmed,
    MonitorLost,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub observed_at: u64,
    pub request_published_at: Option<u64>,
    pub deadline_unconfirmed_at: Option<u64>,
    pub process_exited_at: Option<u64>,
    pub output_complete_at: Option<u64>,
    pub save_confirmed_at: Option<u64>,
    pub monitor_lost_at: Option<u64>,
    pub facts: ProcessObservation,
}
impl Snapshot {
    pub fn classification(&self) -> Classification {
        if self.monitor_lost_at.is_some() {
            Classification::MonitorLost
        } else if self.deadline_unconfirmed_at.is_some() {
            Classification::DeadlineUnconfirmed
        } else if self.process_exited_at.is_some()
            && self.output_complete_at.is_some()
            && self.facts.forwarding_complete
        {
            Classification::Completed
        } else {
            Classification::Pending
        }
    }
}

pub struct Monitor {
    deadline_ms: u64,
    state: Snapshot,
}
impl Monitor {
    pub fn new(deadline_ms: u64) -> Result<Self, MonitorError> {
        if deadline_ms == 0 {
            return Err(MonitorError::InvalidDeadline);
        }
        Ok(Self {
            deadline_ms,
            state: Snapshot::default(),
        })
    }
    fn time(&self, at_ms: u64) -> Result<(), MonitorError> {
        if at_ms < self.state.observed_at {
            Err(MonitorError::ClockRegressed)
        } else {
            Ok(())
        }
    }
    pub fn request_published(&mut self, at_ms: u64) -> Result<(), MonitorError> {
        self.note_request_published(at_ms, at_ms)
    }
    /// Record the actual publication time even when an independent watcher
    /// learns it later; only the observation clock must remain monotonic.
    pub fn note_request_published(
        &mut self,
        published_at: u64,
        observed_at: u64,
    ) -> Result<(), MonitorError> {
        self.time(observed_at)?;
        if published_at > observed_at {
            return Err(MonitorError::InvalidEvidence);
        }
        self.state.observed_at = observed_at;
        self.state.request_published_at.get_or_insert(published_at);
        Ok(())
    }
    pub fn observe(&mut self, at_ms: u64, facts: ProcessObservation) -> Result<(), MonitorError> {
        self.time(at_ms)?;
        let previous = self.state.facts;
        if (previous.exited && !facts.exited)
            || (previous.stdout_eof && !facts.stdout_eof)
            || (previous.stdout_eof && facts.received_bytes != previous.received_bytes)
            || (previous.forwarding_complete && !facts.forwarding_complete)
            || facts.received_bytes < previous.received_bytes
            || (previous.received_hash.is_some() && facts.received_hash != previous.received_hash)
            || (previous.saved.is_some() && facts.saved != previous.saved)
        {
            return Err(MonitorError::FactRegressed);
        }
        if (facts.received_hash.is_some() && !facts.stdout_eof)
            || (facts.forwarding_complete && (!facts.stdout_eof || facts.received_hash.is_none()))
            || (facts.saved == Some(true)
                && (!facts.stdout_eof
                    || facts.received_hash.is_none()
                    || !facts.forwarding_complete))
        {
            return Err(MonitorError::InvalidEvidence);
        }
        self.state.observed_at = at_ms;
        self.state.facts = facts;
        if at_ms >= self.deadline_ms
            && !(facts.exited
                && facts.stdout_eof
                && facts.received_hash.is_some()
                && facts.forwarding_complete)
        {
            self.state.deadline_unconfirmed_at.get_or_insert(at_ms);
        }
        if facts.exited {
            self.state.process_exited_at.get_or_insert(at_ms);
        }
        if facts.stdout_eof && facts.received_hash.is_some() {
            self.state.output_complete_at.get_or_insert(at_ms);
        }
        if facts.saved == Some(true) {
            self.state.save_confirmed_at.get_or_insert(at_ms);
        }
        Ok(())
    }
    pub fn lose_monitor(&mut self, at_ms: u64) -> Result<(), MonitorError> {
        self.time(at_ms)?;
        self.state.observed_at = at_ms;
        self.state.monitor_lost_at.get_or_insert(at_ms);
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        self.state
    }
}
