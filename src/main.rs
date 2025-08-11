use std::{path::Path, time::Duration};

use notify::{self, Config, Event, EventKind, PollWatcher, Watcher};
use notify_rust::{Notification, Timeout, Urgency};

// static mut LAST_NOTIF_LEVEL: Option<BatteryLevel> = None;

#[derive(Debug, PartialEq, Clone, Copy)]
struct BatteryStatus {
        battery_percentage: u8,
        battery_level: BatteryLevel,
        plugged_in: bool,
        last_notif_level: Option<BatteryLevel>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum BatteryLevel {
        High,      // 90-100%
        Normal,    // >40
        Low,       // 25-40%
        VeryLow,   // 15-24%
        Critical,  // <15%
}

impl BatteryStatus {
        fn update(&mut self) {
                let current_battery = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity").unwrap();
                let current_battery = current_battery.trim().parse::<u8>().unwrap();
                let plugged_in = std::fs::read_to_string("/sys/class/power_supply/ADP1/online").unwrap();
                let plugged_in = match plugged_in.trim().parse::<u8>().unwrap() {
                        0 => false,
                        _ => true,
                };
                let battery_level = match current_battery {
                        0..=14   => BatteryLevel::Critical,
                        15..=24  => BatteryLevel::VeryLow,
                        25..=40  => BatteryLevel::Low,
                        89..=100 => BatteryLevel::High,
                        _        => BatteryLevel::Normal
                };

                self.battery_percentage = current_battery;
                self.battery_level = battery_level;
                self.plugged_in = plugged_in;
        }

        fn new() -> Self {
                BatteryStatus {
                        battery_percentage: 89,
                        battery_level: BatteryLevel::Normal,
                        plugged_in: false,
                        last_notif_level: None,
                }
        }
}

fn battery_watch(event: Result<Event, notify::Error>, battery_status: &mut BatteryStatus) {
        let level = battery_status.battery_level;
        let last_notify = battery_status.last_notif_level;
        let plugged_in = battery_status.plugged_in;
        let percentage = battery_status.battery_percentage;

        match event {
                Ok(evt) => if matches!(evt.kind, EventKind::Modify(_)) {
                        let should_notify = match last_notify {
                                None => !matches!(level, BatteryLevel::Normal),
                                Some(last_level) => last_level != level && !matches!(level, BatteryLevel::Normal)
                        };

                        if should_notify {
                                let (status, body, urgency, timeout) = match level {
                                        BatteryLevel::High => (
                                                "High Battery Charge",
                                                "Unplug your computer from power source to prevent the device from overheating",
                                                Urgency::Low,
                                                Timeout::Milliseconds(60_000),
                                        ),
                                        BatteryLevel::Low => (
                                                "Battery Low",
                                                "Connect your computer to a power source as soon as possible",
                                                Urgency::Normal,
                                                Timeout::Milliseconds(120_000) // 2mins
                                        ),
                                        BatteryLevel::VeryLow => (
                                                "Battery Very Low",
                                                "Less than 25% Battery left. Plug your computer in immediately!",
                                                Urgency::Critical,
                                                Timeout::Milliseconds(600_000) // 10mins
                                        ),
                                        _  => (
                                                "Battery Critical", 
                                                "Shutting down in 60 seconds. Press Meta+Ctrl+Shift+A to abort.",
                                                Urgency::Critical,
                                                Timeout::Milliseconds(60_000) // 1min
                                        )
                                };

                                let notification = Notification::new()
                                        .summary(status)
                                        .body(&format!("[{}%] {}", percentage, body))
                                        .urgency(urgency)
                                        .timeout(timeout)
                                        .show()
                                        .unwrap();

                                battery_status.last_notif_level = Some(level);

                                if !plugged_in && matches!(level, BatteryLevel::High) {
                                        notification.close();
                                }

                                if matches!(level, BatteryLevel::Critical) {
                                        std::thread::spawn(|| {
                                                std::thread::sleep(Duration::from_secs(60));
                                                std::process::Command::new("poweroff").spawn().unwrap();
                                        });
                                }
                        }
                        battery_status.update();
                },
                Err(err) => eprintln!("Watch error: {}", err)
        }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let config = Config::default()
                .with_poll_interval(Duration::from_millis(500))
                .with_compare_contents(true);
        let mut watcher = PollWatcher::new(move |event| tx.send(event).unwrap(), config)?;
        let battery_cap_path = Path::new("/sys/class/power_supply/BAT0/capacity");
        let mut battery_status = BatteryStatus::new();

        watcher.watch(battery_cap_path, notify::RecursiveMode::NonRecursive)?;

        for event in rx {
                battery_watch(event, &mut battery_status);
                println!("{:#?}", battery_status)
        }

        Ok(())
}


