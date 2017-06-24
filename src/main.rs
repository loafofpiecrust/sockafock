#![feature(lookup_host)]

extern crate byteorder;

use std::io::{self, Read, Write};
use std::net;
use std::net::{TcpStream, TcpListener, SocketAddr, Ipv4Addr, IpAddr};
use std::thread;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor};

/// Proxy all requests from the given client stream over SOCKS5.
fn proxy(mut client: TcpStream) -> io::Result<()> {
    let peer = client.peer_addr()?;
    println!("Connected to {:?}", peer);
    let data = &mut [0];

    // Socks version
    client.read(data)?;
    if data[0] != 5 {
        // TODO: Error more gracefully?
        panic!("Only SOCKS5 is supported. No SOCKS4 or anything else.");
    }

    // Authentication data
    client.read(data)?;
    let auth_count = data[0];
    // println!("# of auth methods: {:?}", auth_count);

    let mut auth_methods = vec![0; auth_count as usize];
    client.read(&mut auth_methods)?;
    // TODO: Handle other types of auth besides NONE.

    client.write(&[5, 0])?;
    // TODO: Add login capacity here!


    client.read(data)?; // socks ver again
    client.read(data)?;
    let command = data[0];
    // println!("command {:?}", command);
    if command != 1 {
        // Doesn't want to make a TCP connection!
        // TODO: Handle more elegantly
        panic!("Doesn't want to make a tcp connection.");
    }

    client.read(data)?; // Reserved value, "must be 0" supposedly
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
            let mut url = vec![0; data[0] as usize]; // remote url buffer
            client.read(&mut url)?; // retrieve remote url
            client.read(port)?;

            let url = String::from_utf8(url).unwrap();
            let mut host = net::lookup_host(&url)?.next().unwrap();
            host.set_port(80);

            host
        },
        _ => { // IPv6 address
            unimplemented!("IPv6 addresses not supported.")
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
        _ => unimplemented!()
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
                for b in remote2.bytes() {
                    client2.write(&[b.unwrap()]).unwrap();
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

fn main() {
    // TODO: Use CLI argument(s) for port and ip.
    let port = 1080; // proxy port for clients to connect to
    let ip = "127.0.0.1"; // ip for proxy to bind to
    let listener = TcpListener::bind(format!("{}:{}", ip, port)).unwrap();

    // for each connection
    for client in listener.incoming() {
        // TODO: Use a taskpool?
        // Spawn a thread to handle each new connection.
        client.map(|client| thread::spawn(move ||
            proxy(client).expect("Failed to proxy connection.")
        )).expect("Connection failed early!");
    }
}
