//! macOS UI utilities for hold badge indicator and caret tracing
//!
//! This module provides native macOS functionality for:
//! - Displaying a floating red badge indicator during recording
//! - Tracking text caret position via Accessibility API
//! - Falling back to cursor position when caret is unavailable

// Allow Apple-style constant naming (kAX* prefixes) for Accessibility API
#![allow(non_upper_case_globals)]

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use dispatch::Queue;
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Type alias for Objective-C object pointers (compatible with objc crate msg_send!)
type Id = *mut Object;

// Accessibility API bindings (use raw pointers compatible with C FFI)
type AXId = *mut std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCopyAttributeValue(element: AXId, attribute: AXId, value: *mut AXId) -> i32;
    fn AXUIElementCreateSystemWide() -> AXId;
    fn AXValueGetValue(value: AXId, type_: i32, value_ptr: *mut std::ffi::c_void) -> bool;
    fn CFRelease(cf: AXId);
}

// AX constants
const kAXErrorSuccess: i32 = 0;
const kAXFocusedUIElementAttribute: &str = "AXFocusedUIElement";
const kAXRoleAttribute: &str = "AXRole";
const kAXSelectedTextRangeAttribute: &str = "AXSelectedTextRange";
const kAXPositionAttribute: &str = "AXPosition";
const kAXSizeAttribute: &str = "AXSize";

// AXValue types
const kAXValueCGPointType: i32 = 1;
const kAXValueCGSizeType: i32 = 2;
#[allow(dead_code)]
const kAXValueCFRangeType: i32 = 3;

// Window level constants
const NS_STATUS_WINDOW_LEVEL: i64 = 25;

/// Configuration for the hold badge
#[derive(Debug, Clone)]
pub struct HoldBadgeConfig {
    /// Diameter of the badge circle in pixels
    pub diameter: f64,
    /// Offset from caret/cursor position (x, y)
    pub offset: (f64, f64),
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
    /// Badge color (R, G, B, A)
    pub color: (f64, f64, f64, f64),
}

impl Default for HoldBadgeConfig {
    fn default() -> Self {
        Self {
            diameter: 12.0,
            offset: (10.0, -10.0),
            update_interval_ms: 150,
            color: (1.0, 0.0, 0.0, 0.8), // Red with 80% opacity
        }
    }
}

/// Hold badge state
struct HoldBadgeState {
    window: Option<usize>, // Store as usize to make it Send
    timer_running: bool,
    config: HoldBadgeConfig,
}

lazy_static::lazy_static! {
    static ref BADGE_STATE: Arc<Mutex<HoldBadgeState>> = Arc::new(Mutex::new(HoldBadgeState {
        window: None,
        timer_running: false,
        config: HoldBadgeConfig::default(),
    }));
}

/// Check if the currently focused element accepts text input
pub fn focused_element_accepts_text() -> bool {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return false;
        }

        let mut focused_element: AXId = ptr::null_mut();
        let attr_name = CFString::new(kAXFocusedUIElementAttribute);
        let result = AXUIElementCopyAttributeValue(
            system_wide,
            attr_name.as_concrete_TypeRef() as AXId,
            &mut focused_element,
        );

        CFRelease(system_wide);

        if result != kAXErrorSuccess || focused_element.is_null() {
            return false;
        }

        // Get role attribute
        let mut role_value: AXId = ptr::null_mut();
        let role_attr = CFString::new(kAXRoleAttribute);
        let role_result = AXUIElementCopyAttributeValue(
            focused_element,
            role_attr.as_concrete_TypeRef() as AXId,
            &mut role_value,
        );

        CFRelease(focused_element);

        if role_result != kAXErrorSuccess || role_value.is_null() {
            return false;
        }

        // Convert role to string
        let role_str = CFString::wrap_under_get_rule(role_value as *const _);
        let role = role_str.to_string();
        CFRelease(role_value);

        // Check if role indicates text input
        matches!(
            role.as_str(),
            "AXTextArea" | "AXTextField" | "AXComboBox" | "AXTextView" | "AXWebArea"
        )
    }
}

/// Get the current text caret position in screen coordinates
pub fn get_caret_position() -> Option<(f64, f64)> {
    unsafe {
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return None;
        }

        let mut focused_element: AXId = ptr::null_mut();
        let attr_name = CFString::new(kAXFocusedUIElementAttribute);
        let result = AXUIElementCopyAttributeValue(
            system_wide,
            attr_name.as_concrete_TypeRef() as AXId,
            &mut focused_element,
        );

        CFRelease(system_wide);

        if result != kAXErrorSuccess || focused_element.is_null() {
            return None;
        }

        // Get selected text range
        let mut range_value: AXId = ptr::null_mut();
        let range_attr = CFString::new(kAXSelectedTextRangeAttribute);
        let range_result = AXUIElementCopyAttributeValue(
            focused_element,
            range_attr.as_concrete_TypeRef() as AXId,
            &mut range_value,
        );

        if range_result != kAXErrorSuccess || range_value.is_null() {
            CFRelease(focused_element);
            return None;
        }

        // Extract range
        #[repr(C)]
        struct CFRange {
            location: i64,
            length: i64,
        }

        let mut cf_range = CFRange {
            location: 0,
            length: 0,
        };

        let range_ok = AXValueGetValue(
            range_value,
            kAXValueCFRangeType,
            &mut cf_range as *mut _ as *mut std::ffi::c_void,
        );

        CFRelease(range_value);

        if !range_ok {
            CFRelease(focused_element);
            return None;
        }

        // Try to get position and size of the focused element
        let mut position_value: AXId = ptr::null_mut();
        let position_attr = CFString::new(kAXPositionAttribute);
        let position_result = AXUIElementCopyAttributeValue(
            focused_element,
            position_attr.as_concrete_TypeRef() as AXId,
            &mut position_value,
        );

        let mut size_value: AXId = ptr::null_mut();
        let size_attr = CFString::new(kAXSizeAttribute);
        let size_result = AXUIElementCopyAttributeValue(
            focused_element,
            size_attr.as_concrete_TypeRef() as AXId,
            &mut size_value,
        );

        CFRelease(focused_element);

        if position_result != kAXErrorSuccess
            || position_value.is_null()
            || size_result != kAXErrorSuccess
            || size_value.is_null()
        {
            if !position_value.is_null() {
                CFRelease(position_value);
            }
            if !size_value.is_null() {
                CFRelease(size_value);
            }
            return None;
        }

        // Extract position
        let mut position = CGPoint { x: 0.0, y: 0.0 };
        let position_ok = AXValueGetValue(
            position_value,
            kAXValueCGPointType,
            &mut position as *mut _ as *mut std::ffi::c_void,
        );

        CFRelease(position_value);

        // Extract size
        let mut size = CGSize {
            width: 0.0,
            height: 0.0,
        };
        let size_ok = AXValueGetValue(
            size_value,
            kAXValueCGSizeType,
            &mut size as *mut _ as *mut std::ffi::c_void,
        );

        CFRelease(size_value);

        if !position_ok || !size_ok {
            return None;
        }

        // Estimate caret position (top-left of element + small offset)
        // For better accuracy, we'd need to parse the text layout, but this is a reasonable approximation
        Some((position.x, position.y + size.height / 2.0))
    }
}

/// Get the current mouse cursor position in screen coordinates
pub fn get_cursor_position() -> (f64, f64) {
    let mouse_location = NSEvent::mouseLocation();
    (mouse_location.x, mouse_location.y)
}

/// Get the best available position for the badge (caret or cursor)
fn get_badge_position() -> (f64, f64) {
    get_caret_position().unwrap_or_else(get_cursor_position)
}

/// Create the hold badge window
unsafe fn create_badge_window(config: &HoldBadgeConfig) -> Id {
    let ns_window = Class::get("NSWindow").unwrap();
    let ns_view = Class::get("NSView").unwrap();

    // Get initial position
    let (x, y) = get_badge_position();
    let adjusted_x = x + config.offset.0;
    let adjusted_y = y + config.offset.1;

    // Create window frame using CGRect
    let frame = CGRect {
        origin: CGPoint {
            x: adjusted_x,
            y: adjusted_y,
        },
        size: CGSize {
            width: config.diameter,
            height: config.diameter,
        },
    };

    // Create window
    let window: Id = msg_send![ns_window, alloc];
    let style_mask = NSWindowStyleMask::Borderless;
    let backing = NSBackingStoreType::Buffered;
    let window: Id = msg_send![
        window,
        initWithContentRect: frame
        styleMask: style_mask
        backing: backing
        defer: false
    ];

    // Configure window
    let clear_color = NSColor::clearColor();
    let clear_color_ptr = &*clear_color as *const _ as Id;
    let _: () = msg_send![window, setOpaque: false];
    let _: () = msg_send![window, setBackgroundColor: clear_color_ptr];
    let _: () = msg_send![window, setIgnoresMouseEvents: true];
    let _: () = msg_send![window, setLevel: NS_STATUS_WINDOW_LEVEL];
    let collection_behavior = NSWindowCollectionBehavior::CanJoinAllSpaces;
    let _: () = msg_send![window, setCollectionBehavior: collection_behavior];

    // Create content view
    let content_view: Id = msg_send![ns_view, alloc];
    let content_view: Id = msg_send![content_view, initWithFrame: frame];
    let _: () = msg_send![window, setContentView: content_view];

    // Create badge view (custom drawing)
    let badge_view = create_badge_view(config);
    let _: () = msg_send![content_view, addSubview: badge_view];

    window
}

/// Create the circular badge view
unsafe fn create_badge_view(config: &HoldBadgeConfig) -> Id {
    // Try to get existing class first
    let badge_class = if let Some(existing_class) = Class::get("HoldBadgeView") {
        existing_class
    } else {
        // Create a custom view class for drawing the circle
        let superclass = Class::get("NSView").unwrap();
        let mut decl = objc::declare::ClassDecl::new("HoldBadgeView", superclass)
            .expect("Failed to create HoldBadgeView class");

        // Use a simple struct for the rect parameter that's compatible with objc Encode
        extern "C" fn draw_rect(
            _this: &Object,
            _cmd: objc::runtime::Sel,
            rect_ptr: *const std::ffi::c_void,
        ) {
            unsafe {
                // Interpret the rect_ptr as CGRect
                let rect = *(rect_ptr as *const CGRect);

                let ns_bezier_path = Class::get("NSBezierPath").unwrap();
                let path: Id = msg_send![ns_bezier_path, bezierPath];

                // Draw circle
                let center_x = rect.size.width / 2.0;
                let center_y = rect.size.height / 2.0;
                let radius = rect.size.width / 2.0;

                let center = CGPoint {
                    x: center_x,
                    y: center_y,
                };
                let _: () = msg_send![
                    path,
                    appendBezierPathWithArcWithCenter: center
                    radius: radius
                    startAngle: 0.0_f64
                    endAngle: 360.0_f64
                ];

                // Set fill color (red with 80% opacity)
                let color = NSColor::colorWithCalibratedRed_green_blue_alpha(
                    1.0, // R
                    0.0, // G
                    0.0, // B
                    0.8, // A
                );
                let color_ptr = &*color as *const _ as Id;
                let _: () = msg_send![color_ptr, setFill];
                let _: () = msg_send![path, fill];
            }
        }

        // Register with raw pointer type (objc runtime handles the actual struct passing)
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, objc::runtime::Sel, *const std::ffi::c_void),
        );

        decl.register()
    };

    // Create the view
    let view: Id = msg_send![badge_class, alloc];
    let frame = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: config.diameter,
            height: config.diameter,
        },
    };
    let view: Id = msg_send![view, initWithFrame: frame];

    view
}

/// Update the badge window position
unsafe fn update_badge_position(window: Id, config: &HoldBadgeConfig) {
    let (x, y) = get_badge_position();
    let adjusted_x = x + config.offset.0;
    let adjusted_y = y + config.offset.1;

    let new_origin = CGPoint {
        x: adjusted_x,
        y: adjusted_y,
    };
    let _: () = msg_send![window, setFrameOrigin: new_origin];
}

/// Show the hold badge and start position tracking
pub fn show_hold_badge() {
    show_hold_badge_with_config(HoldBadgeConfig::default());
}

/// Internal implementation that must run on the main thread
fn show_hold_badge_impl(config: HoldBadgeConfig) {
    unsafe {
        let mut state = BADGE_STATE.lock().unwrap();

        // Hide existing badge if any
        if let Some(window_ptr) = state.window {
            let window = window_ptr as Id;
            let _: () = msg_send![window, close];
            state.window = None;
        }

        // Create new badge window (MUST be on main thread)
        let window = create_badge_window(&config);
        let _: () = msg_send![window, orderFrontRegardless];

        state.window = Some(window as usize);
        state.config = config.clone();
        state.timer_running = true;

        // Start position update timer
        let update_interval = config.update_interval_ms;

        thread::spawn(move || {
            while BADGE_STATE.lock().unwrap().timer_running {
                thread::sleep(Duration::from_millis(update_interval));

                let state = BADGE_STATE.lock().unwrap();
                if !state.timer_running {
                    break;
                }

                if let Some(window_ptr) = state.window {
                    // Position updates also need main thread
                    Queue::main().exec_async(move || {
                        let window = window_ptr as Id;
                        let state = BADGE_STATE.lock().unwrap();
                        update_badge_position(window, &state.config);
                    });
                }
            }
        });
    }
}

/// Show the hold badge with custom configuration
/// This dispatches to the main thread for thread safety with NSWindow
pub fn show_hold_badge_with_config(config: HoldBadgeConfig) {
    // Dispatch to main thread - NSWindow MUST be created on main thread
    Queue::main().exec_async(move || {
        show_hold_badge_impl(config);
    });
}

/// Hide the hold badge and stop position tracking
/// This dispatches to the main thread for thread safety with NSWindow
pub fn hide_hold_badge() {
    // Stop the timer first (can be done on any thread)
    {
        let mut state = BADGE_STATE.lock().unwrap();
        state.timer_running = false;
    }

    // Dispatch window close to main thread
    Queue::main().exec_async(|| unsafe {
        let mut state = BADGE_STATE.lock().unwrap();
        if let Some(window_ptr) = state.window {
            let window = window_ptr as Id;
            let _: () = msg_send![window, close];
            state.window = None;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_position() {
        let (x, y) = get_cursor_position();
        // Just verify we get some coordinates
        assert!(x >= 0.0);
        assert!(y >= 0.0);
    }

    #[test]
    fn test_focused_element_check() {
        // This will return false in test environment (no GUI)
        // but verifies the function doesn't crash
        let _ = focused_element_accepts_text();
    }

    #[test]
    fn test_badge_config_default() {
        let config = HoldBadgeConfig::default();
        assert_eq!(config.diameter, 12.0);
        assert_eq!(config.offset, (10.0, -10.0));
        assert_eq!(config.update_interval_ms, 150);
    }
}
