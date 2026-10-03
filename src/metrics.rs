//! Métricas de FPS e de recursos do sistema/processo.

use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

pub struct SystemMetrics {
    system: System,
    pid: Pid,
    last_update: Instant,
    pub process_cpu_usage: f32,
    pub process_ram_mb: f64,
    pub global_cpu_usage: f32,
    pub used_ram_gb: f64,
    pub total_ram_gb: f64,
}

impl SystemMetrics {
    pub fn new() -> Self {
        Self {
            system: System::new_with_specifics(
                RefreshKind::new()
                    .with_cpu(sysinfo::CpuRefreshKind::everything())
                    .with_memory(sysinfo::MemoryRefreshKind::everything()),
            ),
            pid: Pid::from_u32(std::process::id()),
            last_update: Instant::now(),
            process_cpu_usage: 0.0,
            process_ram_mb: 0.0,
            global_cpu_usage: 0.0,
            used_ram_gb: 0.0,
            total_ram_gb: 0.0,
        }
    }

    pub fn tick(&mut self) {
        if self.last_update.elapsed() >= Duration::from_millis(500) {
            self.last_update = Instant::now();
            self.system.refresh_cpu_usage();
            self.system.refresh_memory();
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[self.pid]),
                ProcessRefreshKind::new().with_cpu().with_memory(),
            );

            self.global_cpu_usage = self.system.global_cpu_usage();
            self.total_ram_gb = self.system.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0);
            self.used_ram_gb = self.system.used_memory() as f64 / (1024.0 * 1024.0 * 1024.0);

            if let Some(process) = self.system.process(self.pid) {
                self.process_cpu_usage = process.cpu_usage();
                self.process_ram_mb = process.memory() as f64 / (1024.0 * 1024.0);
            }
        }
    }
}

pub struct FpsCounter {
    frame_count: u32,
    last_update: Instant,
    pub current_fps: f64,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            last_update: Instant::now(),
            current_fps: 60.0,
        }
    }

    pub fn tick(&mut self) {
        self.frame_count += 1;
        let elapsed = self.last_update.elapsed();
        if elapsed >= Duration::from_millis(500) {
            self.current_fps = (self.frame_count as f64) / elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_update = Instant::now();
        }
    }
}
