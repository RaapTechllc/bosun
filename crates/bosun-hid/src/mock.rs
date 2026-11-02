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
    /// Fail with an opaque backend error, which is how `hidapi` reports a
    /// physical unplug on every OS.
    Error(String),
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
    input_report_polls: VecDeque<std::result::Result<Vec<u8>, String>>,
    report_descriptor: Option<Vec<u8>>,
    writes: Vec<Vec<u8>>,
    feature_writes: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            info,
            reads: VecDeque::new(),
            feature_reads: VecDeque::new(),
            input_report_polls: VecDeque::new(),
            report_descriptor: None,
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

    /// Queue a read that fails the way a real unplug does: an opaque backend
    /// error rather than [`HidError::Disconnected`].
    #[must_use]
    pub fn push_error(mut self, message: &str) -> Self {
        self.reads
            .push_back(ScriptedRead::Error(message.to_owned()));
        self
    }

    /// Queue the payload a [`Transport::get_feature_report`] call will return.
    #[must_use]
    pub fn push_feature_report(mut self, report: &[u8]) -> Self {
        self.feature_reads.push_back(report.to_vec());
        self
    }

    /// Queue the answer to one [`Transport::get_input_report`] poll.
    ///
    /// Polls are their own script, separate from the interrupt reads queued by
    /// [`push_report`](Self::push_report): a device can answer one channel and
    /// stay silent on the other.
    #[must_use]
    pub fn push_input_report(mut self, report: &[u8]) -> Self {
        self.input_report_polls.push_back(Ok(report.to_vec()));
        self
    }

    /// Queue a [`Transport::get_input_report`] poll that fails with a backend
    /// error, e.g. a device that stalls `GET_REPORT`.
    #[must_use]
    pub fn push_input_report_error(mut self, message: &str) -> Self {
        self.input_report_polls.push_back(Err(message.to_owned()));
        self
    }

    /// Serve this report descriptor from [`Transport::get_report_descriptor`].
    /// Without one, the call fails as an unsupported backend would.
    #[must_use]
    pub fn with_report_descriptor(mut self, descriptor: &[u8]) -> Self {
        self.report_descriptor = Some(descriptor.to_vec());
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
            Some(ScriptedRead::Error(message)) => Err(HidError::Backend(message)),
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

    fn get_input_report(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self.input_report_polls.pop_front() {
            None => Err(HidError::Disconnected),
            Some(Err(message)) => Err(HidError::Backend(message)),
            Some(Ok(report)) => copy_report(&report, buf),
        }
    }

    fn get_report_descriptor(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &self.report_descriptor {
            Some(descriptor) => copy_report(descriptor, buf),
            None => Err(HidError::Backend(
                "this transport has no report descriptor".to_owned(),
            )),
        }
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
    fn a_scripted_backend_error_fails_the_read_like_a_real_unplug() {
        // hidapi reports a pulled cable as an opaque error on every OS, never
        // as `Disconnected`; the mock has to be able to say the same thing.
        let mut transport = mock()
            .push_error("hid_read_timeout: unexpected poll error (device disconnected)")
            .push_report(&[0x01, 0xAA]);
        let mut buf = [0u8; 8];

        match transport.read(&mut buf, TIMEOUT) {
            Err(HidError::Backend(message)) => assert!(message.contains("disconnected")),
            other => panic!("expected a backend error, got {other:?}"),
        }
    }

    #[test]
    fn input_report_polls_are_scripted_independently_of_interrupt_reads() {
        let mut transport = mock()
            .push_report(&[0x01, 0xAA])
            .push_input_report(&[0x01, 0xBB, 0xCC]);
        let mut buf = [0u8; 8];
        buf[0] = 0x01;

        assert_eq!(transport.get_input_report(&mut buf).unwrap(), 3);
        assert_eq!(&buf[..3], &[0x01, 0xBB, 0xCC]);
        // The interrupt script is untouched by polling.
        assert_eq!(transport.reads_remaining(), 1);
        assert!(matches!(
            transport.get_input_report(&mut buf),
            Err(HidError::Disconnected)
        ));
    }

    #[test]
    fn a_scripted_poll_error_surfaces_as_a_backend_error() {
        let mut transport = mock()
            .push_input_report_error("GET_REPORT stalled")
            .push_input_report(&[0x01]);
        let mut buf = [0u8; 8];

        assert!(matches!(
            transport.get_input_report(&mut buf),
            Err(HidError::Backend(message)) if message == "GET_REPORT stalled"
        ));
        assert_eq!(transport.get_input_report(&mut buf).unwrap(), 1);
    }

    #[test]
    fn the_report_descriptor_is_served_only_when_scripted() {
        let mut buf = [0u8; 16];

        assert!(matches!(
            mock().get_report_descriptor(&mut buf),
            Err(HidError::Backend(_))
        ));

        let mut transport = mock().with_report_descriptor(&[0x06, 0x00, 0xFF]);
        assert_eq!(transport.get_report_descriptor(&mut buf).unwrap(), 3);
        assert_eq!(&buf[..3], &[0x06, 0x00, 0xFF]);
        // Served again: a descriptor is a property, not a queue.
        assert_eq!(transport.get_report_descriptor(&mut buf).unwrap(), 3);
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
