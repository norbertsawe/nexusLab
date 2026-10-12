//! Device workers own their ports; the GUI only polls a lock-free SPSC queue.
pub mod scpi;
use nexus_core::{Generator, HeapCons, HeapRb, Waveform};
use ringbuf::traits::{Consumer, Producer, Split};
use serde::Serialize;
pub use serialport as serialport_export;
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use thiserror::Error;

pub const SAMPLE_RATE: f64 = 128_000.0;
#[derive(Debug, Error)]
pub enum HardwareError {
    #[error("serial: {0}")]
    Serial(#[from] serialport::Error),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol: {0}")]
    Protocol(String),
}
#[derive(Clone, Debug, Serialize)]
pub struct PortInfo {
    pub name: String,
    pub description: String,
}
pub fn ports() -> Result<Vec<PortInfo>, HardwareError> {
    let mut out: Vec<_> = serialport::available_ports()?
        .into_iter()
        .map(|p| {
            let description = match p.port_type {
                serialport::SerialPortType::UsbPort(u) => u
                    .product
                    .unwrap_or_else(|| format!("USB {:04X}:{:04X}", u.vid, u.pid)),
                _ => "Serial device".into(),
            };
            PortInfo {
                name: p.port_name,
                description,
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}
pub fn windows_com_names() -> impl Iterator<Item = String> {
    (1..=256).map(|i| format!("COM{i}"))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Framing {
    Lines,
    Float32Le,
}
#[derive(Clone, Debug)]
pub struct SerialConfig {
    pub port: String,
    pub baud: u32,
    pub parity: serialport::Parity,
    pub flow_control: serialport::FlowControl,
    pub framing: Framing,
    pub sample_rate: f64,
}
impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: "COM3".into(),
            baud: 115200,
            parity: serialport::Parity::None,
            flow_control: serialport::FlowControl::None,
            framing: Framing::Lines,
            sample_rate: SAMPLE_RATE,
        }
    }
}
pub struct FrameDecoder {
    framing: Framing,
    pending: Vec<u8>,
}
impl FrameDecoder {
    pub fn new(framing: Framing) -> Self {
        Self {
            framing,
            pending: Vec::new(),
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<f64> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        let consumed = match self.framing {
            Framing::Float32Le => {
                let n = self.pending.len() / 4 * 4;
                for chunk in self.pending[..n].as_chunks::<4>().0 {
                    let v = f32::from_le_bytes(*chunk) as f64;
                    if v.is_finite() {
                        out.push(v);
                    }
                }
                n
            }
            Framing::Lines => {
                let mut end = 0;
                for (i, &b) in self.pending.iter().enumerate() {
                    if b == b'\n' {
                        if let Ok(line) = std::str::from_utf8(&self.pending[end..i]) {
                            if let Ok(v) = line.trim().parse::<f64>() {
                                if v.is_finite() {
                                    out.push(v);
                                }
                            }
                        }
                        end = i + 1;
                    }
                }
                end
            }
        };
        self.pending.drain(..consumed);
        if self.pending.len() > 65536 {
            self.pending.clear();
        }
        out
    }
}
enum Command {
    Mock(Waveform, f64),
    Serial(SerialConfig),
    Disconnect,
}
pub struct Acquisition {
    consumer: HeapCons<[f64; 2]>,
    tx: mpsc::Sender<Command>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
    pub received: Arc<AtomicU64>,
    pub dropped: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
    rate: Arc<AtomicU64>,
}
impl Acquisition {
    pub fn start_mock() -> Self {
        let (mut producer, consumer) = HeapRb::<[f64; 2]>::new(262144).split();
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new("Mock · streaming".into()));
        let received = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let rate = Arc::new(AtomicU64::new(SAMPLE_RATE.to_bits()));
        let worker_rate = rate.clone();
        let (s, st, r, d) = (
            stop.clone(),
            status.clone(),
            received.clone(),
            dropped.clone(),
        );
        let worker = thread::spawn(move || {
            let mut generator = Generator::new(SAMPLE_RATE);
            let mut config: Option<SerialConfig> = None;
            let mut port: Option<Box<dyn serialport::SerialPort>> = None;
            let mut mock = true;
            let mut decoder = FrameDecoder::new(Framing::Lines);
            let mut last = Instant::now();
            let mut retry = Instant::now();
            let mut index = 0u64;
            let mut input = [0u8; 8192];
            while !s.load(Ordering::Relaxed) {
                while let Ok(command) = rx.try_recv() {
                    port = None;
                    match command {
                        Command::Mock(w, f) => {
                            mock = true;
                            config = None;
                            generator.waveform = w;
                            generator.frequency = f;
                            worker_rate.store(SAMPLE_RATE.to_bits(), Ordering::Relaxed);
                            *st.lock().unwrap() = "Mock · streaming".into();
                        }
                        Command::Serial(c) => {
                            mock = false;
                            decoder = FrameDecoder::new(c.framing);
                            worker_rate.store(c.sample_rate.to_bits(), Ordering::Relaxed);
                            config = Some(c);
                            index = 0;
                            retry = Instant::now();
                        }
                        Command::Disconnect => {
                            mock = false;
                            config = None;
                            *st.lock().unwrap() = "Disconnected".into();
                        }
                    }
                    last = Instant::now();
                }
                if mock {
                    let n = (last.elapsed().as_secs_f64() * SAMPLE_RATE) as usize;
                    if n > 0 {
                        last += Duration::from_secs_f64(n as f64 / SAMPLE_RATE);
                        for _ in 0..n.min(16384) {
                            if producer.try_push(generator.sample()).is_err() {
                                d.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        r.fetch_add(n.min(16384) as u64, Ordering::Relaxed);
                    }
                    thread::sleep(Duration::from_millis(2));
                    continue;
                }
                if let Some(c) = &config {
                    if port.is_none() && Instant::now() >= retry {
                        match serialport::new(&c.port, c.baud)
                            .parity(c.parity)
                            .flow_control(c.flow_control)
                            .timeout(Duration::from_millis(20))
                            .open()
                        {
                            Ok(p) => {
                                port = Some(p);
                                *st.lock().unwrap() = format!("{} · connected", c.port);
                            }
                            Err(e) => {
                                *st.lock().unwrap() = format!("Reconnecting {}: {e}", c.port);
                                retry = Instant::now() + Duration::from_secs(1);
                            }
                        }
                    }
                }
                if let Some(p) = port.as_mut() {
                    match p.read(&mut input) {
                        Ok(0) => {
                            port = None;
                            retry = Instant::now() + Duration::from_secs(1);
                            *st.lock().unwrap() = "Connection closed · retrying".into();
                        }
                        Ok(n) => {
                            for y in decoder.feed(&input[..n]) {
                                if producer
                                    .try_push([
                                        index as f64
                                            / f64::from_bits(worker_rate.load(Ordering::Relaxed)),
                                        y,
                                    ])
                                    .is_err()
                                {
                                    d.fetch_add(1, Ordering::Relaxed);
                                }
                                index += 1;
                                r.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            port = None;
                            retry = Instant::now() + Duration::from_secs(1);
                            *st.lock().unwrap() = format!("Connection lost: {e}");
                        }
                    }
                } else {
                    thread::sleep(Duration::from_millis(20));
                }
            }
        });
        Self {
            consumer,
            tx,
            stop,
            status,
            received,
            dropped,
            worker: Some(worker),
            rate,
        }
    }
    pub fn mock(&self, w: Waveform, f: f64) {
        let _ = self.tx.send(Command::Mock(w, f));
    }
    pub fn connect(&self, c: SerialConfig) {
        if !c.sample_rate.is_finite() || c.sample_rate <= 0.0 || c.baud == 0 {
            *self.status.lock().unwrap() = "Invalid sample rate or baud".into();
            return;
        }
        let _ = self.tx.send(Command::Serial(c));
    }
    pub fn sample_rate(&self) -> f64 {
        f64::from_bits(self.rate.load(Ordering::Relaxed))
    }
    /// Borrow initialized queue storage without copying. Consumption occurs after the callback.
    pub fn with_samples<R>(
        &mut self,
        limit: usize,
        f: impl FnOnce(&[[f64; 2]], &[[f64; 2]]) -> R,
    ) -> R {
        let (a, b) = self.consumer.as_slices();
        let na = a.len().min(limit);
        let nb = b.len().min(limit - na);
        let result = f(&a[..na], &b[..nb]);
        self.consumer.skip(na + nb);
        result
    }
    pub fn disconnect(&self) {
        let _ = self.tx.send(Command::Disconnect);
    }
    pub fn status(&self) -> String {
        self.status.lock().unwrap().clone()
    }
    pub fn drain(&mut self, output: &mut Vec<[f64; 2]>, limit: usize) {
        for _ in 0..limit {
            if let Some(x) = self.consumer.try_pop() {
                output.push(x);
            } else {
                break;
            }
        }
    }
}
impl Drop for Acquisition {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(w) = self.worker.take() {
            let _ = w.join();
        }
    }
}

#[derive(Debug, Serialize)]
pub struct UsbInfo {
    pub vendor: u16,
    pub product: u16,
    pub name: String,
}
pub fn usb_devices() -> Result<Vec<UsbInfo>, HardwareError> {
    Ok(nusb::list_devices()?
        .map(|d| UsbInfo {
            vendor: d.vendor_id(),
            product: d.product_id(),
            name: d.product_string().unwrap_or("USB device").into(),
        })
        .collect())
}
/// Claim an interface once; call `read` repeatedly for direct bulk endpoint acquisition.
pub struct UsbBulk {
    interface: nusb::Interface,
    endpoint: u8,
}
impl UsbBulk {
    pub fn open(
        vendor: u16,
        product: u16,
        interface: u8,
        endpoint: u8,
    ) -> Result<Self, HardwareError> {
        if endpoint & 0x80 == 0 {
            return Err(HardwareError::Protocol(
                "bulk IN endpoint must have direction bit 0x80".into(),
            ));
        }
        let device = nusb::list_devices()?
            .find(|d| d.vendor_id() == vendor && d.product_id() == product)
            .ok_or_else(|| HardwareError::Protocol("USB device not found".into()))?
            .open()?;
        Ok(Self {
            interface: device.claim_interface(interface)?,
            endpoint,
        })
    }
    pub async fn read(&self, size: usize) -> Result<Vec<u8>, HardwareError> {
        let result = self
            .interface
            .bulk_in(self.endpoint, nusb::transfer::RequestBuffer::new(size))
            .await;
        result
            .status
            .map_err(|e| HardwareError::Protocol(e.to_string()))?;
        Ok(result.data)
    }
    /// Read HID-style interrupt reports on a claimed, accessible USB interface.
    pub async fn read_interrupt(&self, size: usize) -> Result<Vec<u8>, HardwareError> {
        let result = self
            .interface
            .interrupt_in(self.endpoint, nusb::transfer::RequestBuffer::new(size))
            .await;
        result
            .status
            .map_err(|e| HardwareError::Protocol(e.to_string()))?;
        Ok(result.data)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ScpiCommand {
    pub header: String,
    pub query: bool,
    pub arguments: String,
}
pub fn parse_scpi(input: &str) -> Result<Vec<ScpiCommand>, HardwareError> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote = None;
    for (i, c) in input.char_indices() {
        if matches!(c, '\'' | '"') {
            if quote == Some(c) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(c);
            }
        }
        if c == ';' && quote.is_none() {
            parts.push(&input[start..i]);
            start = i + 1;
        }
        if c == '\0' || c == '\r' || c == '\n' && i + 1 != input.len() {
            return Err(HardwareError::Protocol("embedded control character".into()));
        }
    }
    if quote.is_some() {
        return Err(HardwareError::Protocol(
            "unterminated quoted argument".into(),
        ));
    }
    parts.push(&input[start..]);
    parts
        .into_iter()
        .map(|p| {
            let p = p.trim();
            let (h, a) = p.split_once(char::is_whitespace).unwrap_or((p, ""));
            let query = h.ends_with('?');
            let header = h.trim_end_matches('?');
            if header.is_empty()
                || !header
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '*' | '_'))
            {
                return Err(HardwareError::Protocol("invalid SCPI header".into()));
            }
            Ok(ScpiCommand {
                header: header.to_ascii_uppercase(),
                query,
                arguments: a.trim().into(),
            })
        })
        .collect()
}
pub fn encode_scpi(commands: &[ScpiCommand]) -> String {
    let mut s = commands
        .iter()
        .map(|c| {
            format!(
                "{}{}{}{}",
                c.header,
                if c.query { "?" } else { "" },
                if c.arguments.is_empty() { "" } else { " " },
                c.arguments
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    s.push('\n');
    s
}
pub fn scpi_tcp(address: &str, command: &str) -> Result<String, HardwareError> {
    use std::net::ToSocketAddrs;
    let commands = parse_scpi(command)?;
    let addr = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| HardwareError::Protocol("address did not resolve".into()))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(encode_scpi(&commands).as_bytes())?;
    if !commands.iter().any(|c| c.query) {
        return Ok(String::new());
    }
    let mut result = Vec::new();
    let mut byte = [0u8; 1];
    while result.len() < 65536 {
        if stream.read(&mut byte)? == 0 || byte[0] == b'\n' {
            break;
        }
        result.push(byte[0]);
    }
    String::from_utf8(result).map_err(|e| HardwareError::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_frames() {
        let mut d = FrameDecoder::new(Framing::Lines);
        assert!(d.feed(b"1.").is_empty());
        assert_eq!(d.feed(b"5\nbad\n2\n"), vec![1.5, 2.0]);
        let mut d = FrameDecoder::new(Framing::Float32Le);
        let b = 2.5f32.to_le_bytes();
        assert!(d.feed(&b[..2]).is_empty());
        assert_eq!(d.feed(&b[2..]), vec![2.5]);
    }
    #[test]
    fn scpi_quotes_and_queries() {
        let c = parse_scpi(":SOUR:FREQ 1000;DISP:TEXT \"a;b\";*IDN?").unwrap();
        assert_eq!(c.len(), 3);
        assert!(c[2].query);
        assert_eq!(parse_scpi(&encode_scpi(&c)).unwrap(), c);
        assert!(parse_scpi("FREQ \"broken").is_err());
    }
}
