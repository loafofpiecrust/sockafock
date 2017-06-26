 #![feature(lookup_host)]

extern crate byteorder;

use std::io::{self, Read, Write};
use std::net;
use std::net::{TcpStream, TcpListener, SocketAddr, Ipv4Addr, IpAddr};
use std::thread;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor};
use std::collections::{HashMap};
use std::ascii::{AsciiExt};

#[derive(Clone, Debug)]
struct Server {
    users: Option<HashMap<String, String>>,
}

impl Server {
    fn new() -> Self {
        Self {
            users: None,
        }
    }

    fn add_user(&mut self, user: String, pass: String) {
        match self.users {
            Some(ref mut us) => { us.insert(user, pass); },
            None => {
                let mut us = HashMap::new();
                us.insert(user, pass);
                self.users = Some(us);
            }
        }
    }

    /// Proxy all requests from the given client stream over SOCKS5.
    fn proxy(&self, mut client: TcpStream) -> io::Result<()> {
        let peer = client.peer_addr()?;
        println!("Connected to {:?}", peer);
        let data = &mut [0];

        // Socks version
        client.read(data)?;
        if data[0] != 5 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Only SOCKS5 is supported. No SOCKS4 or anything else."
            ));
        }

        // Authentication data
        client.read(data)?;
        let auth_count = data[0];

        let mut auth_methods = vec![0; auth_count as usize];
        client.read(&mut auth_methods)?;

        if let Some(users) = self.users.as_ref() {
            // User/Pass required.
            if !auth_methods.contains(&2) {
                // No auth isn't an option
                client.write(&[5, 0xFF])?; // failure
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Authentication type mismatch. User/Pass required."
                ));
            }

            client.write(&[5, 2])?; // User/pass

            // Now read client login data.
            client.read(data)?; // socks version
            client.read(data)?; // username length
            let mut username = vec![0; data[0] as usize];
            client.read(&mut username)?;
            client.read(data)?; // password length
            let mut password = vec![0; data[0] as usize];
            client.read(&mut password)?;

            // Server verification response
            if users.get(&String::from_utf8(username).unwrap()) == Some(&String::from_utf8(password).unwrap()) {
                // User matched!
                client.write(&[5, 0])?; // success!
            } else {
                client.write(&[5, 1])?; // failure
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Incorrect username or password."
                ));
            }


        } else {
            // No authentication required
            if !auth_methods.contains(&0) {
                // No auth isn't an option
                println!("failed auth!");
                client.write(&[5, 0xFF])?; // failure
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "Authentication type mismatch. No auth required."
                ));
            }

            client.write(&[5, 0])?; // no auth
        }
        // TODO: Handle other types of auth besides NONE.

        // TODO: Add login capacity here!

        client.read(data)?; // socks ver again
        client.read(data)?;
        let command = data[0];
        if command != 1 {
            // Doesn't want to make a TCP connection!
            // TODO: Handle UDP and other connection types
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "No support for connections other than TCP."
            ));
        }

        client.read(data)?; // Reserved value, "must be 0" according to SOCKS5 spec.
        client.read(data)?;
        let addr_type = data[0];
        let port = &mut [0, 80]; // Default to port 80
        let remote_host = match addr_type {
            1 => { // IPv4 address
                let ip = &mut [0, 0, 0, 0];
                client.read(ip)?;
                client.read(port)?;
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])), 80)
            },
            3 => { // Domain name (URL)
                client.read(data)?; // length of url
                let mut url = vec![0; data[0] as usize]; // buffer for remote url
                client.read(&mut url)?; // retrieve remote url
                client.read(port)?;

                let url = String::from_utf8(url).unwrap();
                // Resolve the URL to an IP address.
                let mut host = net::lookup_host(&url)?.next().unwrap();
                host.set_port(80);

                host
            },
            _ => { // IPv6 address
                unimplemented!("IPv6 addresses not yet supported.")
            }
        };


        // Finally, the server response!
        // TODO: Add IPv6 support.
        client.write(&[5, 0, 0, 1])?; // Always IPv4 address, for now.
        match remote_host {
            SocketAddr::V4(sock) => {
                // Send the (DNS) resolved remote ip address.
                client.write(&sock.ip().octets())?;
            }
            _ => unimplemented!() // IPv6
        }
        client.write(port)?; // remote port

        client.flush()?; // make for sure everything is sent

        // Convert server-order (almost always big endian) u8's to a u16
        let port_num = Cursor::new(&port).read_u16::<BigEndian>()?;

        match remote_host {
            SocketAddr::V4(socket) => {
                let mut remote = TcpStream::connect(format!("{}:{}", socket.ip(), port_num))?;

                // (Finally) Proxy data between the client and remote server.

                let mut client2 = client.try_clone()?;
                let remote2 = remote.try_clone()?;
                // Concurrently tunnel all incoming data from the remote server to the client
                thread::spawn(move || {
                    // TODO: Here, flip all gendered pronouns in English. He <=> She
                    let mut word = String::new();
                    for b in remote2.try_clone().unwrap().bytes() {
                        let b = b.unwrap();
                        let c = b.to_ascii_uppercase() as char;
                        // TODO: Only change over HTTP, _not_ HTTPS. Maybe disable if port == 443
                        // TODO: Find a non-compressed and non-encrypted website to test with.
                        // TODO: I'm sure this could be done better.
                        if b.is_ascii() && c != ' ' && (!word.is_empty() || c == 'H' || c == 'S')  {
                            // We have a letter!
                            word.push(c);
                            if word.starts_with("SHE") {
                                // Print 'He'
                                client2.write(&['H' as u8, 'e' as u8]).unwrap();
                                for _ in 0..3 {
                                    word.remove(0);
                                }
                            } else if word.starts_with("HE") {
                                // Print 'She'
                                client2.write(&['S' as u8, 'h' as u8, 'e' as u8]).unwrap();
                                for _ in 0..2 {
                                    word.remove(0);
                                }
                            } else {
                                // Print the character
                                client2.write(&[b]).unwrap();
                            }
                        } else {
                            client2.write(&[b]).unwrap();
                            word.clear();
                        }
                    }
                });

                // Tunnel all incoming data from the _client_ to the remote server.
                for b in client.bytes() {
                    remote.write(&[b?])?;
                }
            }
            host => unimplemented!("{:?}", host)
        }

        Ok(())
    }
}

fn main() {
    // TODO: Use CLI argument(s) for port and ip.
    let port = 1080; // proxy port for clients to connect to
    let ip = "127.0.0.1"; // ip for proxy to bind to
    let listener = TcpListener::bind(format!("{}:{}", ip, port)).unwrap();

    let server = Server::new();

    // add any users here!

    // for each connection
    for client in listener.incoming() {
        // TODO: Use a taskpool?
        // Spawn a thread to handle each new connection.
        let server = server.clone();
        client.map(|client| thread::spawn(move ||
            server.proxy(client).expect("Failed to proxy connection.")
        )).expect("Connection failed early!");
    }
}


#[test]
fn test_connect() {
}
