use std::io::prelude::*;
use std::io::{self, BufReader};
use std::net::{TcpStream};

use lxi::{LxiHook, LxiDevice};


fn remove_newline(text: &mut Vec<u8>) {
    match text.pop() {
        Some(b'\n') => match text.pop() {
            Some(b'\r') => (),
            Some(c) => text.push(c),
            None => (),
        },
        Some(c) => text.push(c),
        None => (),
    }
}

/// Hook for parsing Keysight SCPI response
pub enum KsHook {}

/// Possible response
#[derive(Debug, Clone, PartialEq)]
pub enum KsData {
    Text(String),
    Bin(Vec<u8>),
}

impl KsData {
    pub fn from_text(v: String) -> Self {
        KsData::Text(v)
    }
    pub fn from_bin(v: Vec<u8>) -> Self {
        KsData::Bin(v)
    }
    pub fn into_text(self) -> Option<String> {
        if let KsData::Text(text) = self {
            Some(text)
        } else {
            None
        }
    }
    pub fn into_bin(self) -> Option<Vec<u8>> {
        if let KsData::Bin(data) = self {
            Some(data)
        } else {
            None
        }
    }
}

impl LxiHook for KsHook {
    type Output = KsData;
    fn read(stream: &mut BufReader<TcpStream>) -> io::Result<Self::Output> {
        let mut buf = vec![0];
        stream.read_exact(&mut buf)
        .and_then(|()| {
            if buf[0] != b'#' {
                // Ascii format
                stream.read_until(b'\n', &mut buf)
                .map(move |_num| { remove_newline(&mut buf); buf })
                .and_then(|buf| String::from_utf8(buf).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "text read: non-utf8 sequence found",
                    )
                })).map(|text| KsData::from_text(text))
            } else {
                // Binary format
                stream.read_exact(&mut buf)
                .and_then(|()| {
                    (buf[0] as char).to_digit(10)
                    .ok_or(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "bin read: second byte is not digit",
                    ))
                })
                .and_then(|n| {
                    buf.resize(n as usize, b'\0');
                    stream.read_exact(&mut buf)
                })
                .and_then(|()| {
                    String::from_utf8_lossy(&buf).parse::<usize>()
                    .map_err(|_e| io::Error::new(
                        io::ErrorKind::InvalidData,
                        "bin read: error parse message size",
                    ))
                })
                .and_then(|n| {
                    buf.resize(n, b'\0');
                    stream.read_exact(&mut buf)
                })
                .and_then(|()| {
                    let mut end = Vec::new();
                    stream.read_until(b'\n', &mut end)
                    .map(|_k| end)
                })
                .and_then(|mut end| {
                    remove_newline(&mut end);
                    if end.len() > 0 {
                        Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "bin read: not only newline after message",
                        ))
                    } else {
                        Ok(KsData::from_bin(buf))
                    }
                })
            }
        })
    }
}

/// Abstract Keysignt LXI device
pub type KsDevice = LxiDevice<KsHook>;


#[cfg(test)]
mod emul;

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread;
    use std::time::{Duration};

    use emul::{Emulator};

    #[test]
    fn emulate() {
        let e = Emulator::new(("localhost", 0)).unwrap();
        let p = e.address().unwrap().port();
        let e = e.run();

        thread::sleep(Duration::from_millis(100));

        {
            let mut d = KsDevice::new((String::from("localhost"), p), None);
            d.connect().unwrap();

            d.send(b"*IDN?").unwrap();
            assert_eq!(d.receive().unwrap(), KsData::from_text(String::from("Emulator")));

            d.send(b"DATA?").unwrap();
            assert_eq!(d.receive().unwrap(), KsData::from_bin(vec![0, 255, 10, 128]));

            d.send(b"*IDN?").unwrap();
            assert_eq!(d.receive().unwrap(), KsData::from_text(String::from("Emulator")));
        }

        e.join().unwrap().unwrap();
    }
}
