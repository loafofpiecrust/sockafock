#![feature(lookup_host)]

extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate socks;
extern crate byteorder;

use std::io::{Read, Write};
use std::net;
use std::net::{TcpStream, TcpListener, SocketAddr, Ipv4Addr, IpAddr};
use std::thread;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor};

fn main() {
    let port = 1080;
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();

    for client in listener.incoming() {
        match client {
            Ok(mut client) => {
                thread::spawn(move || {
                    // Connection!
                    let peer = client.peer_addr().unwrap();
                    println!("connected to {:?}", peer);
                    let data = &mut [0];

                    client.read(data).unwrap();
                    println!("socks version: {:?}", data);

                    // new connection
                    client.read(data).unwrap();
                    let auth_count = data[0];
                    println!("# auth methods: {:?}", auth_count);

                    let mut login = false;
                    for i in 0..auth_count {
                        client.read(data).unwrap();
                        println!("auth method {}: {:?}", i + 1, data);
                        // 0 = no auth
                        // 1 = GSSAPI
                        // 2 = user/pass
                        if data[0] == 2 {
                            login = true;
                        }
                    }

                    let written = client.write(&[5]).unwrap();
                    client.write(&[0]).unwrap();
                    client.flush().unwrap();
                    println!("sent {:?} bytes", written);
                    if !login {
                    } else {
                        // TODO: Add login capacity here!
                    }


                    client.read(data).unwrap(); // socks ver again
                    client.read(data).unwrap();
                    let command = data[0];
                    println!("command {:?}", command);
                    if command != 1 {
                        // Doesn't want to make a TCP connection!
                        return;
                    }

                    client.read(data).unwrap(); // Must be 0
                    println!("must be 0? is {:?}", data);
                    client.read(data).unwrap();
                    let addr_ty = data[0];
                    println!("addr ty {:?}", addr_ty);
                    let port = &mut [0, 0];
                    let remote_host = match addr_ty {
                        1 => { // IPv4 address
                            let ip = &mut [0, 0, 0, 0];
                            client.read(ip).unwrap();
                            println!("going to ip {:?}", ip);
                            client.read(port).unwrap();
                            // How to use port?
                            println!("port! {:?}", port);
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])), 80)
                        },
                        3 => { // Domain name
                            let len = &mut [0];
                            client.read(len).unwrap();
                            let mut url = vec![0; len[0] as usize];
                            client.read(&mut url).unwrap();
                            let url = String::from_utf8(url).unwrap();
                            println!("going to addr {:?}", url);
                            client.read(port).unwrap();
                            // How to use port?
                            println!("port! {:?}", port);
                            let host = net::lookup_host(&url).unwrap().next().unwrap();
                            println!("at host {:?}", host);

                            host
                        },
                        _ => { // IPv6 address
                            println!("foodled");
                            unimplemented!()
                        }
                    };


                    // Finally, the server response!
                    client.write(&[5, 0, 0, 1]).unwrap(); // Always IPv4 addr for now
                    match remote_host {
                        SocketAddr::V4(sock) => {
                            client.write(&sock.ip().octets()).unwrap();
                        }
                        _ => unimplemented!()
                    }
                    client.write(port).unwrap(); // server port

                    client.flush().unwrap();

                    let port_num = {
                        let mut rdr = Cursor::new(&port);
                        rdr.read_u16::<BigEndian>().unwrap()
                    };

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
