use std::ffi::c_double;
use std::os::raw::{c_int, c_ulong};

#[cfg(feature = "stress-test")]
use crate::core::StressConfig;
#[cfg(feature = "stress-test")]
use crate::ffi::{INITIALIZED, STRESS_CONFIG, STRESS_MODE};
#[cfg(feature = "stress-test")]
use std::sync::atomic::Ordering;

/// Enable random preemption stress testing (only with "stress-test" feature)
///
/// This function enables stress testing with random preemption before lock
/// acquisitions to increase deadlock probability.
///
/// # Arguments
/// * `probability` - Probability of preemption (0.0-1.0)
/// * `min_delay_us` - Minimum delay duration in microseconds
/// * `max_delay_us` - Maximum delay duration in microseconds
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub unsafe extern "C" fn deloxide_enable_random_stress(
    probability: c_double,
    min_delay_us: c_ulong,
    max_delay_us: c_ulong,
) -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(1, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = Some(crate::core::stress::StressConfig {
                preemption_probability: probability,
                min_delay_us,
                max_delay_us,
                preempt_after_release: true,
            });
        }

        0
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        -1
    }
}

/// Enable component-based stress testing (only with "stress-test" feature)
///
/// This function enables stress testing with targeted delays based on lock
/// acquisition patterns to increase deadlock probability.
///
/// # Arguments
/// * `min_delay_us` - Minimum delay duration in microseconds
/// * `max_delay_us` - Maximum delay duration in microseconds
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
#[allow(unused_variables)]
pub unsafe extern "C" fn deloxide_enable_component_stress(
    min_delay_us: c_ulong,
    max_delay_us: c_ulong,
) -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(2, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = Some(StressConfig {
                preemption_probability: 0.8, // High probability for component-based mode
                min_delay_us,
                max_delay_us,
                preempt_after_release: true,
            });
        }

        0
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        -1
    }
}

/// Disable stress testing (only with "stress-test" feature)
///
/// This function disables any previously enabled stress testing mode.
///
/// # Returns
/// * `0` on success
/// * `1` if already initialized
/// * `-1` if stress-test feature is not enabled
///
/// # Safety
/// This function writes to mutable static variables and should be called before initialization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn deloxide_disable_stress() -> c_int {
    #[cfg(feature = "stress-test")]
    {
        if INITIALIZED.load(Ordering::SeqCst) {
            return 1; // Already initialized
        }

        STRESS_MODE.store(0, Ordering::SeqCst);

        unsafe {
            STRESS_CONFIG = None;
        }

        0
    }

    #[cfg(not(feature = "stress-test"))]
    {
        // Return error if stress-test feature is not enabled
        -1
    }
}
