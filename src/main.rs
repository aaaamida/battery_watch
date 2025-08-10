use std::{path::Path, time::Duration};

use notify::{self, Config, Event, EventKind, PollWatcher, Watcher};
use notify_rust::{Notification, Timeout, Urgency};

static mut LAST_NOTIF_LEVEL: Option<BatteryLevel> = None;

#[derive(Debug, PartialEq, Clone, Copy)]
enum BatteryLevel {
        Normal,    // >40
        Low,       // 25-40%
        VeryLow,   // 15-24%
        Critical,  // <15%
}

fn battery_level(percent: u8) -> BatteryLevel {
        match percent {
                0..=14  => BatteryLevel::Critical,
                15..=24 => BatteryLevel::VeryLow,
                25..=40 => BatteryLevel::Low,
                _       => BatteryLevel::Normal
        }
}

fn battery_watch(event: Result<Event, notify::Error>) {
        let cap = std::fs::read_to_string("/sys/class/power_supply/BAT0/capacity").unwrap();
        let cap = cap.trim().parse::<u8>().unwrap();
        let level = battery_level(cap);

        match event {
                Ok(evt) => if matches!(evt.kind, EventKind::Modify(_)) {
                        let should_notify = unsafe {
                                match LAST_NOTIF_LEVEL {
                                        None => !matches!(level, BatteryLevel::Normal),
                                        Some(last_level) => last_level != level && !matches!(level, BatteryLevel::Normal)
                                }
                        };

                        if should_notify {
                                let (status, body, urgency, timeout) = match level {
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

                                Notification::new()
                                        .summary(status)
                                        .body(&format!("[{}%] {}", cap, body))
                                        .urgency(urgency)
                                        .timeout(timeout)
                                        .show()
                                        .unwrap();

                                unsafe { LAST_NOTIF_LEVEL = Some(level); }

                                if matches!(level, BatteryLevel::Critical) {
                                        std::thread::spawn(|| {
                                                std::thread::sleep(Duration::from_secs(60));
                                                std::process::Command::new("poweroff").spawn().unwrap();
                                        });
                                }
                        }
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

        let cap = Path::new("/sys/class/power_supply/BAT0/capacity");
        watcher.watch(cap, notify::RecursiveMode::NonRecursive)?;

        for event in rx {
                battery_watch(event);
        }

        Ok(())
}


