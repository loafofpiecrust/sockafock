#![feature(lookup_host)]

extern crate byteorder;

use std::io::{Read, Write};
use std::net;
use std::net::{TcpStream, TcpListener, SocketAddr, Ipv4Addr, IpAddr};
use std::thread;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor};

fn main() {
    let port = 1080; // proxy port for clients to connect to
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

    // for each connection
    for client in listener.incoming() {
        match client {
            Ok(mut client) => {
                // TODO: Use a taskpool?
                // Spawn a thread to handle each new connection.
                thread::spawn(move || {
                    let peer = client.peer_addr().unwrap();
                    println!("Connected to {:?}", peer);
                    let data = &mut [0];

                    // Socks version
                    client.read(data).unwrap();
                    if data[0] != 5 {
                        // TODO: Error more gracefully?
                        panic!("Only SOCKS5 is supported. No SOCKS4 or anything else.");
                    }

                    // Authentication data
                    client.read(data).unwrap();
                    let auth_count = data[0];
                    // println!("# of auth methods: {:?}", auth_count);

                    let mut auth_methods = vec![0; auth_count as usize];
                    client.read(&mut auth_methods).unwrap();
                    // TODO: Handle other types of auth besides NONE.

                    client.write(&[5, 0]).unwrap();
                    // TODO: Add login capacity here!


                    client.read(data).unwrap(); // socks ver again
                    client.read(data).unwrap();
                    let command = data[0];
                    // println!("command {:?}", command);
                    if command != 1 {
                        // Doesn't want to make a TCP connection!
                        // TODO: Handle more elegantly
                        return;
                    }

                    client.read(data).unwrap(); // Reserved value, "must be 0" supposedly
                    client.read(data).unwrap();
                    let addr_type = data[0];
                    let port = &mut [0, 80];
                    let remote_host = match addr_type {
                        1 => { // IPv4 address
                            let ip = &mut [0, 0, 0, 0];
                            client.read(ip).unwrap();
                            client.read(port).unwrap();
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])), 80)
                        },
                        3 => { // Domain name (URL)
                            client.read(data).unwrap(); // length of url
                            let mut url = vec![0; data[0] as usize]; // remote url buffer
                            client.read(&mut url).unwrap(); // retrieve remote url
                            client.read(port).unwrap();

                            let url = String::from_utf8(url).unwrap();
                            let mut host = net::lookup_host(&url).unwrap().next().unwrap();
                            host.set_port(80);

                            host
                        },
                        _ => { // IPv6 address
                            unimplemented!("IPv6 addresses not yet supported.")
                        }
                    };


                    // Finally, the server response!
                    client.write(&[5, 0, 0, 1]).unwrap(); // Always IPv4 addr for now
                    match remote_host {
                        SocketAddr::V4(sock) => {
                            // Send the (DNS) resolved remote ip address.
                            client.write(&sock.ip().octets()).unwrap();
                        }
                        _ => unimplemented!()
                    }
                    client.write(port).unwrap(); // remote port

                    client.flush().unwrap(); // make for sure everything is sent

                    // Convert server-order (almost always big endian) u8's to a u16
                    let port_num = Cursor::new(&port).read_u16::<BigEndian>().unwrap();

                    match remote_host {
                        SocketAddr::V4(sock) => {
                            let mut remote = TcpStream::connect(format!("{}:{}", sock.ip(), port_num)).unwrap();
                            println!("proxy to {}:{}", sock.ip(), port_num);

                            let mut client2 = client.try_clone().unwrap();
                            let remote2 = remote.try_clone().unwrap();
                            thread::spawn(move || {
                                for b in remote2.bytes() {
                                    client2.write(&[b.unwrap()]).unwrap();
                                }
                            });

                            for b in client.bytes() {
                                remote.write(&[b.unwrap()]).unwrap();
                            }
                        }
                        _ => unimplemented!()
                    }
                });
            },
            Err(e) => {
                println!("Connection failed! {:?}", e);
            }
        }
    }
}
