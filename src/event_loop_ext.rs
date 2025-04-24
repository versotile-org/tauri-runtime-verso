use tao::event_loop::EventLoopWindowTarget as TaoEventLoopWindowTarget;
use tauri_runtime::{Error, Result, dpi::PhysicalPosition, monitor::Monitor};

pub trait TaoEventLoopWindowTargetExt {
    fn tauri_primary_monitor(&self) -> Option<Monitor>;
    fn tauri_monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor>;
    fn tauri_available_monitors(&self) -> Vec<Monitor>;
    fn tauri_cursor_position(&self) -> Result<PhysicalPosition<f64>>;
}

impl<T> TaoEventLoopWindowTargetExt for TaoEventLoopWindowTarget<T> {
    fn tauri_primary_monitor(&self) -> Option<Monitor> {
        self.primary_monitor().map(tao_monitor_to_tauri_monitor)
    }

    fn tauri_monitor_from_point(&self, x: f64, y: f64) -> Option<Monitor> {
        self.monitor_from_point(x, y)
            .map(tao_monitor_to_tauri_monitor)
    }

    fn tauri_available_monitors(&self) -> Vec<Monitor> {
        self.available_monitors()
            .map(tao_monitor_to_tauri_monitor)
            .collect()
    }

    fn tauri_cursor_position(&self) -> Result<PhysicalPosition<f64>> {
        let position = self
            .cursor_position()
            .map_err(|_| Error::FailedToGetCursorPosition)?;
        Ok(position)
    }
}

pub fn tao_monitor_to_tauri_monitor(monitor: tao::monitor::MonitorHandle) -> Monitor {
    Monitor {
        name: monitor.name(),
        position: monitor.position(),
        scale_factor: monitor.scale_factor(),
        size: monitor.size(),
    }
}
