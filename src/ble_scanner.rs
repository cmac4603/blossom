use bt_hci::cmd::le::LeSetScanParams;
use bt_hci::controller::ControllerCmdSync;
use core::cell::RefCell;
use core::default::Default;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use esp_println::println;
use heapless::{Deque, String, Vec};
use trouble_host::prelude::*;

/// Max number of connections
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

pub async fn run<C>(controller: C)
where
    C: Controller + ControllerCmdSync<LeSetScanParams>,
{
    // Using a fixed "random" address can be useful for testing. In real scenarios, one would
    // use e.g. the MAC 6 byte array as the address (how to get that varies by the platform).
    let address: Address = Address::random([0xff, 0x8f, 0x1b, 0x05, 0xe4, 0xff]);

    println!("Our address = {:?}", address);

    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let stack = trouble_host::new(controller, &mut resources).set_random_address(address);
    let host = stack.build();
    let central = host.central;
    let mut runner = host.runner;

    let printer = Printer {
        seen: RefCell::new(Deque::new()),
    };
    let mut scanner = Scanner::new(central);
    println!("Running BT scan...");
    let _ = join(runner.run_with_handler(&printer), async {
        let mut config = ScanConfig::default();
        config.active = true;
        config.phys = PhySet::M1;
        config.interval = Duration::from_secs(1);
        config.window = Duration::from_secs(1);
        let mut _session = scanner.scan(&config).await.unwrap();
        // Scan forever
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    })
    .await;
}

struct Printer {
    seen: RefCell<Deque<BdAddr, 128>>,
}

impl EventHandler for Printer {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        let mut seen = self.seen.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            if seen.iter().find(|b| b.raw() == report.addr.raw()).is_none() {
                let ad_structure = AdStructure::decode(report.data);
                println!("----");
                println!("New device advertising:");
                for ad in ad_structure {
                    if let Ok(adv) = ad {
                        println!("advertisement: {:?}", adv);
                        match adv {
                            AdStructure::CompleteLocalName(name) => {
                                let mut da_name = Vec::<u8, 64>::new();
                                da_name.extend_from_slice(name).unwrap();
                                let local_name = String::from_utf8(da_name).unwrap();
                                println!("{local_name}");
                            },
                            AdStructure::ManufacturerSpecificData {
                                company_identifier,
                                payload: _,
                            } => {
                                // flipper = 3625
                                if company_identifier == 3625 {
                                    println!("flipper found!!!!!");
                                }
                            }
                            _ => (),
                        }
                    }
                }
                if seen.is_full() {
                    seen.pop_front();
                }
                seen.push_back(report.addr).unwrap();
            }
        }
    }
}
