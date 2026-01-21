// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Configuration parsing for embassy-sim

use raft_core::observer::EventLevel;

/// Observer level configuration
const CONFIG_JSON: &str = include_str!("../config.json");

/// Parse observer level from config.json
pub fn get_observer_level() -> EventLevel {
    // Simple JSON parsing for observer_level field
    // Format: { "observer_level": "Essential" | "Info" | "Debug" | "Trace" | "None" }

    if let Some(start) = CONFIG_JSON.find("\"observer_level\"") {
        if let Some(colon_pos) = CONFIG_JSON[start..].find(':') {
            let value_start = start + colon_pos + 1;
            if let Some(quote_start) = CONFIG_JSON[value_start..].find('\"') {
                let value_start = value_start + quote_start + 1;
                if let Some(quote_end) = CONFIG_JSON[value_start..].find('\"') {
                    let level_str = &CONFIG_JSON[value_start..value_start + quote_end];
                    return match level_str.trim() {
                        "None" => EventLevel::None,
                        "Essential" => EventLevel::Essential,
                        "Info" => EventLevel::Info,
                        "Debug" => EventLevel::Debug,
                        "Trace" => EventLevel::Trace,
                        _ => {
                            info!(
                                "Unknown observer level '{}', defaulting to Essential",
                                level_str
                            );
                            EventLevel::Essential
                        }
                    };
                }
            }
        }
    }

    info!("Could not parse observer_level from config, defaulting to Essential");
    EventLevel::Essential
}
