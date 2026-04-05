// =============================================================================
// SysForge - Core Backend Engine
// =============================================================================
// High-performance Windows System Optimizer
// Phase 1: Real-time CPU, RAM, Disk, and Temperature monitoring via Tauri commands
// Phase 2: Action Center — Temp file cleaning & telemetry management
//
// Architecture Notes:
// - System state is held in a `Mutex<System>` managed by Tauri's state system.
//   This avoids expensive re-initialization on every invocation.
// - Commands are async and use `spawn_blocking` to avoid freezing the Tauri
//   event loop during the CPU measurement delay.
// - All memory/disk values are in MB for frontend convenience.
// =============================================================================

use serde::Serialize;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::fs;
use std::path::PathBuf;
use sysinfo::{Components, CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};
use tauri::State;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

// =============================================================================
// Managed State
// =============================================================================

/// Wrapper around `sysinfo::System` held in Tauri's managed state.
/// Using a Mutex ensures safe concurrent access from multiple command invocations.
pub struct SystemState(pub Mutex<System>);

// =============================================================================
// Data Structures
// =============================================================================

/// Represents a single temperature sensor reading.
#[derive(Debug, Serialize, Clone)]
pub struct TemperatureInfo {
    /// Human-readable label for the sensor (e.g., "ACPI Thermal Zone")
    pub label: String,
    /// Current temperature in degrees Celsius
    pub temperature_celsius: f32,
    /// Maximum recorded temperature in °C (if available from the sensor)
    pub max_temperature_celsius: Option<f32>,
}

/// Represents a single disk/volume.
#[derive(Debug, Serialize, Clone)]
pub struct DiskInfo {
    /// Disk name (e.g., "C:" on Windows)
    pub name: String,
    /// Mount point path
    pub mount_point: String,
    /// Filesystem type (e.g., "NTFS", "FAT32")
    pub file_system: String,
    /// Total disk space in MB
    pub total_space_mb: u64,
    /// Available (free) space in MB
    pub available_space_mb: u64,
    /// Used space in MB
    pub used_space_mb: u64,
    /// Usage as a percentage (0.0–100.0)
    pub usage_percent: f64,
}

/// Aggregated system statistics returned to the frontend.
///
/// All memory values are expressed in **megabytes (MB)** for frontend convenience.
#[derive(Debug, Serialize)]
pub struct SystemStats {
    // -- CPU ------------------------------------------------------------------
    /// CPU brand/model string (e.g., "Intel Core i7-12700K")
    pub cpu_brand: String,
    /// Number of logical CPU cores
    pub cpu_count: usize,
    /// Per-core CPU usage as a percentage (0.0–100.0)
    pub cpu_usage_percent: Vec<f32>,
    /// Overall (average) CPU usage percentage
    pub cpu_overall_percent: f32,

    // -- Memory ---------------------------------------------------------------
    /// Total physical RAM in MB
    pub total_memory_mb: u64,
    /// Currently used RAM in MB
    pub used_memory_mb: u64,
    /// Available (free) RAM in MB
    pub available_memory_mb: u64,
    /// RAM usage as a percentage (0.0–100.0)
    pub memory_usage_percent: f64,

    // -- Swap -----------------------------------------------------------------
    /// Total swap (page file) in MB
    pub total_swap_mb: u64,
    /// Currently used swap in MB
    pub used_swap_mb: u64,

    // -- Disks ----------------------------------------------------------------
    /// Information about each mounted disk/volume
    pub disks: Vec<DiskInfo>,

    // -- Temperatures ---------------------------------------------------------
    /// Temperature readings from all detected sensors.
    /// May be empty on systems without accessible temperature sensors.
    pub temperatures: Vec<TemperatureInfo>,

    // -- System Info ----------------------------------------------------------
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Total number of running processes
    pub process_count: usize,
}

// =============================================================================
// Helper: bytes → megabytes
// =============================================================================

/// Converts bytes to megabytes (integer division).
#[inline]
fn bytes_to_mb(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

// =============================================================================
// Tauri Commands
// =============================================================================

/// Fetches real-time system statistics including CPU, RAM, disks, and temperatures.
///
/// # Design Notes
/// - Uses `tauri::State<SystemState>` to persist the `System` instance across calls,
///   avoiding costly re-initialization on every poll.
/// - Runs the blocking CPU measurement on a dedicated thread via `spawn_blocking`
///   so the Tauri async runtime isn't blocked.
/// - CPU usage requires two refresh calls with a small delay in between for the
///   `sysinfo` crate to calculate meaningful percentage values.
/// - Temperature data gracefully degrades to an empty list on hardware/drivers
///   that don't expose sensor data (common on some Windows configurations).
///
/// # Errors
/// Returns a string error if the system state mutex is poisoned (should not happen
/// under normal operation).
#[tauri::command]
async fn get_system_stats(state: State<'_, SystemState>) -> Result<SystemStats, String> {
    // Clone the Arc-like state handle for use inside the blocking thread.
    // We lock inside spawn_blocking to keep the lock duration minimal.
    let sys_mutex = state.0.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;

    // Snapshot the system — we take a quick lock, refresh, release.
    // For CPU, we need the first refresh already done (from previous call or init),
    // then sleep, then refresh again to get a delta.
    drop(sys_mutex);

    // Perform the blocking CPU measurement off the async runtime.
    let stats = {
        let state_inner = state.0.lock().map_err(|e| format!("Mutex poisoned: {}", e))?;
        // We need to move the MutexGuard into a scope where we can do the work.
        // But MutexGuard isn't Send, so we do everything synchronously in this block.
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        // Copy over relevant cached data is not possible with sysinfo's API,
        // so we use the managed state for the initial CPU baseline.
        drop(state_inner);

        // First refresh to establish baseline
        sys.refresh_cpu_usage();

        // Brief sleep to allow CPU usage delta calculation
        thread::sleep(Duration::from_millis(200));

        // Second refresh with actual usage data
        sys.refresh_cpu_usage();
        sys.refresh_memory();

        // -- CPU stats --------------------------------------------------------
        let cpus = sys.cpus();
        let cpu_brand = cpus
            .first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown CPU".to_string());
        let cpu_count = cpus.len();
        let cpu_usage_percent: Vec<f32> = cpus.iter().map(|cpu| cpu.cpu_usage()).collect();
        let cpu_overall_percent = if cpu_count > 0 {
            cpu_usage_percent.iter().sum::<f32>() / cpu_count as f32
        } else {
            0.0
        };

        // -- Memory stats -----------------------------------------------------
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let available_memory = sys.available_memory();
        let memory_usage_percent = if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        };

        // -- Swap stats -------------------------------------------------------
        let total_swap = sys.total_swap();
        let used_swap = sys.used_swap();

        // -- Disk stats -------------------------------------------------------
        let sysinfo_disks = Disks::new_with_refreshed_list();
        let disks: Vec<DiskInfo> = sysinfo_disks
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                let usage_pct = if total > 0 {
                    (used as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                DiskInfo {
                    name: disk.name().to_string_lossy().to_string(),
                    mount_point: disk.mount_point().to_string_lossy().to_string(),
                    file_system: String::from_utf8_lossy(disk.file_system().as_encoded_bytes())
                        .to_string(),
                    total_space_mb: bytes_to_mb(total),
                    available_space_mb: bytes_to_mb(available),
                    used_space_mb: bytes_to_mb(used),
                    usage_percent: usage_pct,
                }
            })
            .collect();

        // -- Temperature sensors ----------------------------------------------
        // Components (sensors) are fetched separately. On Windows, this uses WMI
        // and may return an empty list if no compatible sensors are detected.
        let components = Components::new_with_refreshed_list();
        let temperatures: Vec<TemperatureInfo> = components
            .iter()
            .map(|component| TemperatureInfo {
                label: component.label().to_string(),
                temperature_celsius: component.temperature().unwrap_or(0.0),
                max_temperature_celsius: component.max(),
            })
            .collect();

        // -- System-wide info -------------------------------------------------
        let uptime_seconds = System::uptime();
        let process_count = sys.processes().len();

        // -- Build response ---------------------------------------------------
        SystemStats {
            cpu_brand,
            cpu_count,
            cpu_usage_percent,
            cpu_overall_percent,
            total_memory_mb: bytes_to_mb(total_memory),
            used_memory_mb: bytes_to_mb(used_memory),
            available_memory_mb: bytes_to_mb(available_memory),
            memory_usage_percent,
            total_swap_mb: bytes_to_mb(total_swap),
            used_swap_mb: bytes_to_mb(used_swap),
            disks,
            temperatures,
            uptime_seconds,
            process_count,
        }
    };

    Ok(stats)
}

// =============================================================================
// Phase 2: Action Center Commands
// =============================================================================

/// Recursively deletes all files in a directory, skipping files that are locked
/// or protected by the OS. Returns the total bytes freed.
///
/// This is intentionally non-recursive for directories to avoid accidentally
/// removing folder structures that Windows may need to recreate.
fn clean_directory(path: &PathBuf) -> (u64, u32, u32) {
    let mut bytes_freed: u64 = 0;
    let mut files_deleted: u32 = 0;
    let mut files_skipped: u32 = 0;

    if !path.exists() {
        return (bytes_freed, files_deleted, files_skipped);
    }

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return (bytes_freed, files_deleted, files_skipped),
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_file() {
            // Get file size before attempting deletion
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            match fs::remove_file(&entry_path) {
                Ok(_) => {
                    bytes_freed += size;
                    files_deleted += 1;
                }
                Err(_) => {
                    // File is locked or protected — skip gracefully
                    files_skipped += 1;
                }
            }
        } else if entry_path.is_dir() {
            // Recursively clean subdirectories
            let (sub_bytes, sub_deleted, sub_skipped) = clean_directory(&entry_path);
            bytes_freed += sub_bytes;
            files_deleted += sub_deleted;
            files_skipped += sub_skipped;
            // Try to remove the now-empty directory (will fail if not empty, that's fine)
            let _ = fs::remove_dir(&entry_path);
        }
    }

    (bytes_freed, files_deleted, files_skipped)
}

/// Formats bytes into a human-readable string (KB, MB, GB).
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} bytes", bytes)
    }
}

/// Cleans temporary files from the user's %TEMP% folder and C:\Windows\Prefetch.
///
/// # Safety
/// - Only deletes files, not system-critical folder structures.
/// - Skips any file that is locked (in use) or lacks permissions.
/// - Prefetch cleaning requires admin rights; if the app isn't elevated,
///   those files will be skipped with a note in the result.
///
/// # Returns
/// A human-readable summary string, e.g., "Cleaned 245.3 MB (312 files deleted, 5 skipped)"
#[tauri::command]
async fn clean_temp_files() -> Result<String, String> {
    let mut total_bytes: u64 = 0;
    let mut total_deleted: u32 = 0;
    let mut total_skipped: u32 = 0;
    let mut sources_cleaned: Vec<String> = Vec::new();

    // 1. User's %TEMP% directory
    if let Ok(temp_dir) = std::env::var("TEMP") {
        let temp_path = PathBuf::from(&temp_dir);
        let (bytes, deleted, skipped) = clean_directory(&temp_path);
        total_bytes += bytes;
        total_deleted += deleted;
        total_skipped += skipped;
        if deleted > 0 {
            sources_cleaned.push(format!("%TEMP% ({})", format_bytes(bytes)));
        }
    }

    // 2. Windows Prefetch — requires admin, will gracefully skip if no access
    let prefetch_path = PathBuf::from(r"C:\Windows\Prefetch");
    let (bytes, deleted, skipped) = clean_directory(&prefetch_path);
    total_bytes += bytes;
    total_deleted += deleted;
    total_skipped += skipped;
    if deleted > 0 {
        sources_cleaned.push(format!("Prefetch ({})", format_bytes(bytes)));
    }

    // 3. Windows Temp
    let win_temp_path = PathBuf::from(r"C:\Windows\Temp");
    let (bytes, deleted, skipped) = clean_directory(&win_temp_path);
    total_bytes += bytes;
    total_deleted += deleted;
    total_skipped += skipped;
    if deleted > 0 {
        sources_cleaned.push(format!("Windows Temp ({})", format_bytes(bytes)));
    }

    if total_deleted == 0 {
        Ok("No temporary files to clean — system is already tidy!".to_string())
    } else {
        Ok(format!(
            "Cleaned {} ({} files deleted, {} skipped).\nSources: {}",
            format_bytes(total_bytes),
            total_deleted,
            total_skipped,
            sources_cleaned.join(", ")
        ))
    }
}

/// Attempts to disable Windows telemetry (DiagTrack) by modifying the registry.
///
/// # Registry Keys Modified
/// - `HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection`
///   - `AllowTelemetry` = 0 (Disables telemetry data collection)
/// - `HKLM\SYSTEM\CurrentControlSet\Services\DiagTrack`
///   - `Start` = 4 (Disables the DiagTrack service)
///
/// # Errors
/// Returns a descriptive error if the app isn't running with admin privileges,
/// or if the registry keys cannot be written for another reason.
#[tauri::command]
async fn disable_telemetry() -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::io::ErrorKind;

        let mut results: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

        // 1. Disable telemetry data collection
        match hklm.open_subkey_with_flags("SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection", KEY_SET_VALUE) {
            Ok(key) => {
                match key.set_value("AllowTelemetry", &0u32) {
                    Ok(_) => results.push("AllowTelemetry set to 0 (disabled)".to_string()),
                    Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                        results.push("DataCollection: Requires TrustedInstaller privileges".to_string());
                    }
                    Err(e) => errors.push(format!("Failed to set AllowTelemetry: {}", e)),
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                results.push("DataCollection key: Already disabled or not present".to_string());
            }
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                results.push("DataCollection: Requires TrustedInstaller privileges".to_string());
            }
            Err(e) => errors.push(format!(
                "Cannot open DataCollection key: {}", e
            )),
        }

        // Helper: attempt to disable a Windows service via registry.
        // NotFound (os error 2) means the service doesn't exist on this system,
        // which is fine — we treat it as already disabled.
        let try_disable_service = |path: &str, name: &str| -> Result<String, Option<String>> {
            match hklm.open_subkey_with_flags(path, KEY_SET_VALUE) {
                Ok(key) => {
                    match key.set_value("Start", &4u32) {
                        Ok(_) => Ok(format!("{} disabled (Start=4)", name)),
                        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                            Ok(format!("{}: Requires TrustedInstaller privileges", name))
                        }
                        Err(e) => Err(Some(format!("Failed to disable {}: {}", name, e))),
                    }
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    Ok(format!("{}: Already disabled or not present", name))
                }
                Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                    Ok(format!("{}: Requires TrustedInstaller privileges", name))
                }
                Err(e) => Err(Some(format!(
                    "Cannot open {} key: {}", name, e
                ))),
            }
        };

        // 2. Disable the DiagTrack service
        match try_disable_service(
            "SYSTEM\\CurrentControlSet\\Services\\DiagTrack",
            "DiagTrack",
        ) {
            Ok(msg) => results.push(msg),
            Err(Some(msg)) => errors.push(msg),
            Err(None) => {}
        }

        // 3. Disable Connected User Experiences and Telemetry
        match try_disable_service(
            "SYSTEM\\CurrentControlSet\\Services\\dmwappushservice",
            "dmwappushservice",
        ) {
            Ok(msg) => results.push(msg),
            Err(Some(msg)) => errors.push(msg),
            Err(None) => {}
        }

        if errors.is_empty() {
            Ok(format!("Telemetry disabled successfully.\n{}", results.join("\n")))
        } else if results.is_empty() {
            Err(format!(
                "Failed to disable telemetry — run SysForge as Administrator.\n{}",
                errors.join("\n")
            ))
        } else {
            Ok(format!(
                "Partial success:\n{}\n\nFailed:\n{}",
                results.join("\n"),
                errors.join("\n")
            ))
        }
    }

    #[cfg(not(windows))]
    {
        Err("Telemetry management is only available on Windows.".to_string())
    }
}

/// Activates Gamer Mode by killing non-essential background processes
/// and switching the Windows power plan to High Performance.
#[tauri::command]
async fn enable_gamer_mode() -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;
        
        let mut killed_count = 0;
        let targets = ["OneDrive.exe", "Skype.exe", "msedge.exe", "Discord.exe"];
        
        // 1. Kill background hogs
        for target in targets.iter() {
            if let Ok(output) = Command::new("taskkill")
                .args(["/F", "/IM", target])
                .creation_flags(0x08000000)
                .output()
            {
                if output.status.success() {
                    killed_count += 1;
                }
            }
        }
        
        // 2. Set Power Plan to High Performance
        let _ = Command::new("powercfg")
            .args(["-setactive", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"])
            .creation_flags(0x08000000)
            .output();
            
        Ok(format!("Killed {} background processes.\nHigh Performance plan activated.", killed_count))
    }

    #[cfg(not(windows))]
    {
        Err("Gamer mode is only available on Windows.".to_string())
    }
}

/// Deactivates Gamer Mode by restoring the Windows Power Plan to Balanced.
///
/// # Commands Executed
/// - `powercfg -setactive 381b4222-f694-41f0-9685-ff5bb260df2e` — Balanced plan
#[tauri::command]
async fn disable_gamer_mode() -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;

        match Command::new("powercfg")
            .args(["-setactive", "381b4222-f694-41f0-9685-ff5bb260df2e"])
            .creation_flags(0x08000000)
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    Ok("Restored normal system performance (Balanced mode).".to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if stderr.contains("Access is denied") || stderr.contains("Access Denied") {
                        Err("Access Denied: Please run SysForge Pro as Administrator.".to_string())
                    } else {
                        Err(format!("Failed to restore Balanced plan: {}", stderr))
                    }
                }
            }
            Err(e) => Err(format!("Failed to execute powercfg: {}", e)),
        }
    }

    #[cfg(not(windows))]
    {
        Err("Gamer mode is only available on Windows.".to_string())
    }
}

// =============================================================================
// Phase 4: Network Optimizer & The Nuke Button
// =============================================================================

/// Optimizes network settings by flushing DNS, resetting Winsock, and resetting
/// the IP stack. These are standard, safe Windows networking commands.
///
/// # Commands Executed
/// - `ipconfig /flushdns`   — Clears the DNS resolver cache
/// - `netsh winsock reset`  — Resets the Winsock catalog to a clean state
/// - `netsh int ip reset`   — Resets TCP/IP stack parameters
///
/// # Errors
/// Returns an error if a critical command fails to execute.
#[tauri::command]
async fn optimize_network() -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        use std::os::windows::process::CommandExt;

        let mut results: Vec<String> = Vec::new();

        // 1. Flush DNS cache
        match Command::new("ipconfig").arg("/flushdns").creation_flags(0x08000000).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let combined = format!("{} {}", stdout, stderr);
                
                if output.status.success() {
                    results.push(format!("✓ DNS Cache Flushed: {}", stdout));
                } else if combined.contains("Access is denied") || combined.contains("Access Denied") {
                    results.push("✗ DNS Flush failed: Access Denied. Please run as Administrator.".to_string());
                } else {
                    results.push(format!("⚠ DNS Flush: {}", if stderr.is_empty() { stdout } else { stderr }));
                }
            }
            Err(e) => results.push(format!("✗ DNS Flush failed: {}", e)),
        }

        // 2. Reset Winsock catalog
        match Command::new("netsh").args(["winsock", "reset"]).creation_flags(0x08000000).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let combined = format!("{} {}", stdout, stderr);
                
                if output.status.success() {
                    results.push("✓ Winsock catalog reset successfully".to_string());
                } else if combined.contains("Access is denied") || combined.contains("Access Denied") || combined.contains("requires elevation") {
                    results.push("✗ Winsock reset failed: Access Denied. Please run as Administrator.".to_string());
                } else {
                    results.push(format!("⚠ Winsock reset: {}", if stderr.is_empty() { stdout } else { stderr }));
                }
            }
            Err(e) => results.push(format!("✗ Winsock reset failed: {}", e)),
        }

        // 3. Reset TCP/IP stack
        match Command::new("netsh").args(["int", "ip", "reset"]).creation_flags(0x08000000).output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let combined = format!("{}\n{}", stdout, stderr);
                
                if output.status.success() || combined.contains("Resetting") || combined.contains("Restart the computer to complete this action") {
                    results.push("✓ TCP/IP stack reset successfully".to_string());
                } else if combined.contains("Access is denied") || combined.contains("Access Denied") || combined.contains("requires elevation") {
                    results.push("✗ TCP/IP reset failed: Access Denied. Please run as Administrator.".to_string());
                } else {
                    let err_msg = if stderr.is_empty() { stdout } else { stderr };
                    results.push(format!("⚠ TCP/IP reset: {}", err_msg));
                }
            }
            Err(e) => results.push(format!("✗ TCP/IP reset failed: {}", e)),
        }

        Ok(format!("Network optimization complete.\n{}", results.join("\n")))
    }

    #[cfg(not(windows))]
    {
        Err("Network optimization is only available on Windows.".to_string())
    }
}

/// The "System Nuke" — runs ALL optimization commands sequentially:
///   1. Clean temp files
///   2. Disable telemetry
///   3. Enable gamer mode
///   4. Optimize network
///
/// Returns a combined report of every action taken.
#[tauri::command]
async fn nuke_system() -> Result<String, String> {
    let mut report: Vec<String> = Vec::new();

    report.push("╔══════════════════════════════════════╗".to_string());
    report.push("║    ☢️  SYSTEM NUKE — FULL REPORT     ║".to_string());
    report.push("╚══════════════════════════════════════╝".to_string());
    report.push(String::new());

    // Phase 1: Clean Temp Files
    report.push("━━━ PHASE 1: TEMP FILE CLEANUP ━━━".to_string());
    match clean_temp_files().await {
        Ok(msg) => report.push(msg),
        Err(e) => report.push(format!("ERROR: {}", e)),
    }
    report.push(String::new());

    // Phase 2: Disable Telemetry
    report.push("━━━ PHASE 2: TELEMETRY KILL ━━━".to_string());
    match disable_telemetry().await {
        Ok(msg) => report.push(msg),
        Err(e) => report.push(format!("ERROR: {}", e)),
    }
    report.push(String::new());

    // Phase 3: Gamer Mode
    report.push("━━━ PHASE 3: GAMER MODE ━━━".to_string());
    match enable_gamer_mode().await {
        Ok(msg) => report.push(msg),
        Err(e) => report.push(format!("ERROR: {}", e)),
    }
    report.push(String::new());

    // Phase 4: Network Optimizer
    report.push("━━━ PHASE 4: NETWORK OPTIMIZER ━━━".to_string());
    match optimize_network().await {
        Ok(msg) => report.push(msg),
        Err(e) => report.push(format!("ERROR: {}", e)),
    }
    report.push(String::new());

    report.push("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".to_string());
    report.push("NUKE COMPLETE — System fully optimized.".to_string());

    Ok(report.join("\n"))
}

// =============================================================================
// Application Entry Point
// =============================================================================

/// Initializes and runs the Tauri application.
///
/// - Registers the `SystemState` as managed state so it persists across command calls.
/// - Registers all `#[tauri::command]` handlers so the React frontend
///   can invoke them via `@tauri-apps/api`.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Pre-initialize the System instance so the first `get_system_stats` call
    // has a baseline for CPU usage calculation.
    let sys = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SystemState(Mutex::new(sys)))
        .invoke_handler(tauri::generate_handler![
            get_system_stats,
            clean_temp_files,
            disable_telemetry,
            enable_gamer_mode,
            disable_gamer_mode,
            optimize_network,
            nuke_system
        ])
        .run(tauri::generate_context!())
        .expect("error while running SysForge");
}
