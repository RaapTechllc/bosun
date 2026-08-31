//! A scripted in-memory [`Transport`] so everything above this crate is
//! testable with no hardware attached.

use std::collections::VecDeque;
use std::time::Duration;

use crate::device::DeviceInfo;
use crate::error::{HidError, Result};
use crate::transport::{ReadOutcome, Transport};

/// One scripted answer to a [`Transport::read`] call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScriptedRead {
    /// Deliver this report to the caller.
    Report(Vec<u8>),
    /// Report that the timeout elapsed.
    Timeout,
    /// Report that the device went away.
    Disconnect,
}

/// A [`Transport`] that replays a script and records everything written to it.
///
/// Once the read script is exhausted every further read reports
/// [`HidError::Disconnected`], which models an unplug and keeps a reconnect
/// loop from spinning forever against an empty mock.
#[derive(Clone, Debug)]
pub struct MockTransport {
    info: DeviceInfo,
    reads: VecDeque<ScriptedRead>,
    feature_reads: VecDeque<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    feature_writes: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            info,
            reads: VecDeque::new(),
            feature_reads: VecDeque::new(),
            writes: Vec::new(),
            feature_writes: Vec::new(),
        }
    }

    /// Queue an input report.
    #[must_use]
    pub fn push_report(mut self, report: &[u8]) -> Self {
        self.reads.push_back(ScriptedRead::Report(report.to_vec()));
        self
    }

    /// Queue an elapsed timeout.
    #[must_use]
    pub fn push_timeout(mut self) -> Self {
        self.reads.push_back(ScriptedRead::Timeout);
        self
    }

    /// Queue a disconnect.
    #[must_use]
    pub fn push_disconnect(mut self) -> Self {
        self.reads.push_back(ScriptedRead::Disconnect);
        self
    }

    /// Queue the payload a [`Transport::get_feature_report`] call will return.
    #[must_use]
    pub fn push_feature_report(mut self, report: &[u8]) -> Self {
        self.feature_reads.push_back(report.to_vec());
        self
    }

    /// Output reports written so far, in order.
    pub fn writes(&self) -> &[Vec<u8>] {
        &self.writes
    }

    /// Feature reports sent so far, in order.
    pub fn feature_writes(&self) -> &[Vec<u8>] {
        &self.feature_writes
    }

    /// Scripted reads not yet consumed.
    pub fn reads_remaining(&self) -> usize {
        self.reads.len()
    }
}

impl Transport for MockTransport {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn read(&mut self, buf: &mut [u8], _timeout: Duration) -> Result<ReadOutcome> {
        match self.reads.pop_front() {
            None | Some(ScriptedRead::Disconnect) => Err(HidError::Disconnected),
            Some(ScriptedRead::Timeout) => Ok(ReadOutcome::Timeout),
            Some(ScriptedRead::Report(report)) => {
                copy_report(&report, buf).map(ReadOutcome::Report)
            }
        }
    }

    fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.writes.push(data.to_vec());
        Ok(data.len())
    }

    fn send_feature_report(&mut self, data: &[u8]) -> Result<()> {
        self.feature_writes.push(data.to_vec());
        Ok(())
    }

    fn get_feature_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        let report = self
            .feature_reads
            .pop_front()
            .ok_or(HidError::Disconnected)?;
        copy_report(&report, buf)
    }
}

fn copy_report(report: &[u8], buf: &mut [u8]) -> Result<usize> {
    if buf.len() < report.len() {
        return Err(HidError::BufferTooSmall {
            expected: report.len(),
            actual: buf.len(),
        });
    }

    buf[..report.len()].copy_from_slice(report);
    Ok(report.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(10);

    fn mock() -> MockTransport {
        MockTransport::new(DeviceInfo {
            path: "mock".to_owned(),
            vendor_id: 0x046D,
            product_id: 0xC21C,
            usage_page: 0xFF00,
            ..DeviceInfo::default()
        })
    }

    #[test]
    fn scripted_reports_arrive_in_order() {
        let mut transport = mock()
            .push_report(&[0x01, 0x80, 0x80, 0, 0, 0, 0, 0])
            .push_report(&[0x01, 0x80, 0x80, 1, 0, 0, 0, 0]);
        let mut buf = [0u8; 8];

        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Report(8)
        );
        assert_eq!(buf, [0x01, 0x80, 0x80, 0, 0, 0, 0, 0]);

        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Report(8)
        );
        assert_eq!(buf, [0x01, 0x80, 0x80, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn a_timeout_consumes_only_its_own_script_entry() {
        let mut transport = mock().push_timeout().push_report(&[0x01, 0xAA]);
        let mut buf = [0u8; 8];

        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Timeout
        );
        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Report(2)
        );
        assert_eq!(&buf[..2], &[0x01, 0xAA]);
    }

    #[test]
    fn an_exhausted_script_reports_a_disconnect() {
        let mut transport = mock().push_report(&[0x01, 0xAA]);
        let mut buf = [0u8; 8];

        transport.read(&mut buf, TIMEOUT).unwrap();

        assert!(matches!(
            transport.read(&mut buf, TIMEOUT),
            Err(HidError::Disconnected)
        ));
        // Still disconnected on a retry: reconnect means re-opening, not
        // retrying the dead handle.
        assert!(matches!(
            transport.read(&mut buf, TIMEOUT),
            Err(HidError::Disconnected)
        ));
    }

    #[test]
    fn a_scripted_disconnect_interrupts_a_pending_script() {
        let mut transport = mock().push_disconnect().push_report(&[0x01, 0xAA]);
        let mut buf = [0u8; 8];

        assert!(matches!(
            transport.read(&mut buf, TIMEOUT),
            Err(HidError::Disconnected)
        ));
    }

    #[test]
    fn a_short_buffer_is_refused_rather_than_truncating_a_report() {
        let mut transport = mock().push_report(&[0x01, 0x02, 0x03, 0x04]);
        let mut buf = [0u8; 2];

        assert!(matches!(
            transport.read(&mut buf, TIMEOUT),
            Err(HidError::BufferTooSmall {
                expected: 4,
                actual: 2
            })
        ));
    }

    #[test]
    fn a_longer_buffer_reports_only_the_bytes_received() {
        let mut transport = mock().push_report(&[0x01, 0x02]);
        let mut buf = [0xFFu8; 8];

        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Report(2)
        );
        assert_eq!(&buf[..2], &[0x01, 0x02]);
        // Bytes past the report are the caller's to interpret; the length is
        // the only claim the transport makes.
        assert_eq!(&buf[2..], &[0xFF; 6]);
    }

    #[test]
    fn writes_are_recorded_verbatim_and_separately_from_feature_reports() {
        let mut transport = mock();

        assert_eq!(transport.write(&[0x03, 0x00, 0x01]).unwrap(), 3);
        transport
            .send_feature_report(&[0x07, 0xFF, 0x00, 0x00])
            .unwrap();

        assert_eq!(transport.writes(), [vec![0x03, 0x00, 0x01]]);
        assert_eq!(transport.feature_writes(), [vec![0x07, 0xFF, 0x00, 0x00]]);
    }

    #[test]
    fn feature_reads_are_queued_and_run_out_independently_of_input_reports() {
        let mut transport = mock()
            .push_report(&[0x01, 0xAA])
            .push_feature_report(&[0x07, 0x11, 0x22, 0x33]);
        let mut buf = [0u8; 8];

        assert_eq!(transport.get_feature_report(&mut buf).unwrap(), 4);
        assert_eq!(&buf[..4], &[0x07, 0x11, 0x22, 0x33]);
        assert!(matches!(
            transport.get_feature_report(&mut buf),
            Err(HidError::Disconnected)
        ));

        // The input script is untouched by feature traffic.
        assert_eq!(transport.reads_remaining(), 1);
    }

    #[test]
    fn the_transport_is_usable_as_a_trait_object() {
        // Higher layers hold `Box<dyn Transport>`, so object safety is part of
        // the contract, not an implementation detail.
        let mut transport: Box<dyn Transport> = Box::new(mock().push_report(&[0x01, 0xAA]));
        let mut buf = [0u8; 8];

        assert_eq!(transport.info().path, "mock");
        assert_eq!(
            transport.read(&mut buf, TIMEOUT).unwrap(),
            ReadOutcome::Report(2)
        );
    }
}
